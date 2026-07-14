// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod connection;
mod file_transfer;
mod relay_client;

use base64::Engine;
use capture::ScreenCapture;
use connection::{ConnectionManager, SessionRole};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub connection_manager: Arc<Mutex<ConnectionManager>>,
    pub screen_capture: Arc<Mutex<ScreenCapture>>,
}

// ── Viewer commands ────────────────────────────────────────────

/// Viewer (Mac): start a relay session after signaling succeeds
#[tauri::command]
async fn start_viewing(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    server_addr: String,
    peer_id: String,
) -> Result<String, String> {
    let mut manager = state.connection_manager.lock().await;
    let session_id = manager
        .start_viewing(&server_addr, &peer_id)
        .await
        .map_err(|e| e.to_string())?;

    // Spawn task to receive frames from relay and emit to frontend
    let manager_arc = state.connection_manager.clone();
    tokio::spawn(async move {
        loop {
            let mut mgr = manager_arc.lock().await;
            if mgr.role != SessionRole::Viewer || !mgr.relay.is_connected() {
                break;
            }
            match mgr.relay.recv_mut().await {
                Ok((relay_client::MSG_FRAME, data)) => {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                    let _ = app.emit("remote-frame", b64);
                }
                Ok((relay_client::MSG_INPUT, _data)) => {
                    // Input events from target not needed on viewer side
                }
                Ok(_) => {} // unknown message type
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            }
        }
    });

    Ok(session_id)
}

// ── Target commands ────────────────────────────────────────────

/// Target (Linux): join relay session and start streaming
#[tauri::command]
async fn start_serving(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    server_addr: String,
    session_id: String,
    viewer_peer: String,
) -> Result<(), String> {
    let mut manager = state.connection_manager.lock().await;
    manager
        .start_serving(&server_addr, &session_id, &viewer_peer)
        .await
        .map_err(|e| e.to_string())?;

    // Start screen capture
    let mut capture = state.screen_capture.lock().await;
    capture.start().map_err(|e| e.to_string())?;
    drop(capture);

    // Spawn continuous capture + send loop
    let manager_arc = state.connection_manager.clone();
    let capture_arc = state.screen_capture.clone();
    tokio::spawn(async move {
        loop {
            let mut mgr = manager_arc.lock().await;
            if mgr.role != SessionRole::Target {
                break;
            }
            let mut cap = capture_arc.lock().await;
            match cap.capture_frame() {
                Ok(frame) => {
                    if mgr.relay.send_mut(relay_client::MSG_FRAME, &frame).await.is_err() {
                        log::info!("Relay send failed, stopping capture");
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Capture error: {}", e);
                }
            }
            drop(cap);
            drop(mgr);
            // Run uncapped — actual speed depends on capture+encode
            tokio::task::yield_now().await;
        }
    });

    // Input receiver: non-blocking poll (100ms interval, try_lock, 1ms timeout)
    let manager_arc2 = state.connection_manager.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let mut mgr = match manager_arc2.try_lock() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if mgr.role != SessionRole::Target || !mgr.relay.is_connected() {
                break;
            }
            if let Ok(Ok((relay_client::MSG_INPUT, data))) = tokio::time::timeout(
                tokio::time::Duration::from_millis(1),
                mgr.relay.recv_mut(),
            ).await {
                if let Ok(event) = serde_json::from_slice::<serde_json::Value>(&data) {
                    #[cfg(target_os = "linux")]
                    simulate_input(&event);
                }
            }
        }
    });

    Ok(())
}

// ── Common commands ────────────────────────────────────────────

#[tauri::command]
async fn disconnect_session(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.connection_manager.lock().await;
    manager.disconnect();
    let mut capture = state.screen_capture.lock().await;
    capture.stop().map_err(|e| e.to_string())
}

#[tauri::command]
async fn send_input_event(
    state: tauri::State<'_, AppState>,
    event_type: String,
    key_code: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    button: Option<String>,
) -> Result<(), String> {
    let mut manager = state.connection_manager.lock().await;
    let data = serde_json::json!({
        "type": event_type,
        "key_code": key_code,
        "x": x,
        "y": y,
        "button": button,
    });
    let bytes = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
    manager.relay.send_mut(relay_client::MSG_INPUT, &bytes).await.map_err(|e| e.to_string())
}

/// Simulate keyboard/mouse input on target using xdotool CLI
#[cfg(target_os = "linux")]
fn simulate_input(event: &serde_json::Value) {
    use std::process::Command;
    let etype = event["type"].as_str().unwrap_or("");

    match etype {
        "keyDown" => {
            if let Some(key) = event["key_code"].as_str() {
                let key_name = key_to_xdotool(key);
                let _ = Command::new("xdotool").args(["keydown", &key_name]).output();
            }
        }
        "keyUp" => {
            if let Some(key) = event["key_code"].as_str() {
                let key_name = key_to_xdotool(key);
                let _ = Command::new("xdotool").args(["keyup", &key_name]).output();
            }
        }
        "mouseMove" => {
            if let (Some(x), Some(y)) = (event["x"].as_f64(), event["y"].as_f64()) {
                let _ = Command::new("xdotool")
                    .args(["mousemove", "--", &format!("{}", x as i32), &format!("{}", y as i32)])
                    .output();
            }
        }
        "mouseDown" => {
            let btn = event["button"].as_str().unwrap_or("left");
            let _ = Command::new("xdotool").args(["mousedown", btn]).output();
        }
        "mouseUp" => {
            let btn = event["button"].as_str().unwrap_or("left");
            let _ = Command::new("xdotool").args(["mouseup", btn]).output();
        }
        _ => {}
    }
}

/// Convert browser KeyboardEvent.code to xdotool key name
#[cfg(target_os = "linux")]
fn key_to_xdotool(code: &str) -> &str {
    match code {
        "Enter" => "Return",
        "Escape" => "Escape",
        "Backspace" => "BackSpace",
        "Tab" => "Tab",
        "Space" => "space",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "ShiftLeft" | "ShiftRight" | "Shift" => "Shift_L",
        "ControlLeft" | "ControlRight" | "Control" => "Control_L",
        "AltLeft" | "AltRight" | "Alt" => "Alt_L",
        "MetaLeft" | "MetaRight" | "Meta" => "Super_L",
        "Delete" => "Delete",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "Prior",
        "PageDown" => "Next",
        "CapsLock" => "Caps_Lock",
        "F1" => "F1", "F2" => "F2", "F3" => "F3", "F4" => "F4",
        "F5" => "F5", "F6" => "F6", "F7" => "F7", "F8" => "F8",
        "F9" => "F9", "F10" => "F10", "F11" => "F11", "F12" => "F12",
        "Minus" => "minus", "Equal" => "equal",
        "BracketLeft" => "bracketleft", "BracketRight" => "bracketright",
        "Backslash" => "backslash", "Semicolon" => "semicolon",
        "Quote" => "apostrophe", "Comma" => "comma", "Period" => "period",
        "Slash" => "slash", "Backquote" => "grave",
        "Digit0" => "0", "Digit1" => "1", "Digit2" => "2", "Digit3" => "3",
        "Digit4" => "4", "Digit5" => "5", "Digit6" => "6", "Digit7" => "7",
        "Digit8" => "8", "Digit9" => "9",
        "KeyA" => "a", "KeyB" => "b", "KeyC" => "c", "KeyD" => "d",
        "KeyE" => "e", "KeyF" => "f", "KeyG" => "g", "KeyH" => "h",
        "KeyI" => "i", "KeyJ" => "j", "KeyK" => "k", "KeyL" => "l",
        "KeyM" => "m", "KeyN" => "n", "KeyO" => "o", "KeyP" => "p",
        "KeyQ" => "q", "KeyR" => "r", "KeyS" => "s", "KeyT" => "t",
        "KeyU" => "u", "KeyV" => "v", "KeyW" => "w", "KeyX" => "x",
        "KeyY" => "y", "KeyZ" => "z",
        _ => code,
    }
}

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState {
                connection_manager: Arc::new(Mutex::new(ConnectionManager::new())),
                screen_capture: Arc::new(Mutex::new(ScreenCapture::new())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_viewing,
            start_serving,
            disconnect_session,
            send_input_event,
        ])
        .run(tauri::generate_context!())
        .expect("error while running rem0te client");
}
