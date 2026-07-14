// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod connection;
mod file_transfer;
mod relay_client;

use anyhow::Context;
use base64::Engine;
use capture::ScreenCapture;
use connection::{ConnectionManager, SessionRole};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub connection_manager: Arc<Mutex<ConnectionManager>>,
    pub screen_capture: Arc<Mutex<ScreenCapture>>,
    pub relay_writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
    pub relay_reader: Arc<Mutex<Option<OwnedReadHalf>>>,
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

    // Extract reader/writer from relay
    let (reader, writer) = {
        let relay = std::mem::replace(&mut manager.relay, relay_client::RelayClient::new());
        relay.into_halves()
    };
    *state.relay_writer.lock().await = writer;
    *state.relay_reader.lock().await = reader;

    // Spawn frame receiver task
    let reader_arc = state.relay_reader.clone();
    let manager_arc = state.connection_manager.clone();
    tokio::spawn(async move {
        loop {
            {
                let mgr = manager_arc.lock().await;
                if mgr.role != SessionRole::Viewer { break; }
            }
            let mut reader_opt = reader_arc.lock().await;
            if let Some(ref mut r) = *reader_opt {
                let mut len_buf = [0u8; 4];
                if r.read_exact(&mut len_buf).await.is_err() { break; }
                let total_len = u32::from_be_bytes(len_buf) as usize;
                if total_len == 0 || total_len > 10_000_000 { break; } // safety
                let mut buf = vec![0u8; total_len];
                if r.read_exact(&mut buf).await.is_err() { break; }
                if buf.len() < 9 { continue; }
                let msg_type = buf[0];
                if msg_type == relay_client::MSG_FRAME {
                    let plen = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
                    let payload = &buf[5..(5+plen).min(buf.len())];
                    let b64 = base64::engine::general_purpose::STANDARD.encode(payload);
                    let _ = app.emit("remote-frame", b64);
                }
            } else {
                break;
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
    _app: tauri::AppHandle,
    server_addr: String,
    session_id: String,
    viewer_peer: String,
) -> Result<(), String> {
    let mut manager = state.connection_manager.lock().await;
    manager
        .start_serving(&server_addr, &session_id, &viewer_peer)
        .await
        .map_err(|e| e.to_string())?;

    // Extract writer from relay
    let (_, writer) = {
        let relay = std::mem::replace(&mut manager.relay, relay_client::RelayClient::new());
        relay.into_halves()
    };
    *state.relay_writer.lock().await = writer;

    // ── Rust capture with mozjpeg (reliable, ~8 FPS full HD) ──
    let writer_arc = state.relay_writer.clone();
    let manager_arc = state.connection_manager.clone();
    let capture_arc = state.screen_capture.clone();
    capture_arc.lock().await.start().unwrap_or(());

    tokio::spawn(async move {
        let mut frame_count: u64 = 0;
        loop {
            {
                let mgr = manager_arc.lock().await;
                if mgr.role != SessionRole::Target { break; }
            }
            let mut cap = capture_arc.lock().await;
            if let Ok(frame) = cap.capture_frame() {
                frame_count += 1;
                let mut w = writer_arc.lock().await;
                if let Some(ref mut writer) = *w {
                    let total_len = (1 + 4 + frame.len()) as u32;
                    let mut hdr = [0u8; 9];
                    hdr[..4].copy_from_slice(&total_len.to_be_bytes());
                    hdr[4] = relay_client::MSG_FRAME;
                    hdr[5..9].copy_from_slice(&(frame.len() as u32).to_be_bytes());
                    if writer.write_all(&hdr).await.is_err()
                        || writer.write_all(&frame).await.is_err()
                    {
                        log::info!("Relay closed after {} frames", frame_count);
                        break;
                    }
                }
            }
            drop(cap);
        }
    });

    Ok(())
}

#[cfg(target_os = "linux")]
async fn start_ffmpeg_pipe(
    writer_arc: Arc<tokio::sync::Mutex<Option<OwnedWriteHalf>>>,
    manager_arc: Arc<tokio::sync::Mutex<ConnectionManager>>,
) -> anyhow::Result<()> {
    use tokio::process::Command;
    use tokio::io::AsyncReadExt;

    // ffmpeg: capture X11 → MJPEG → pipe stdout
    log::info!("Launching ffmpeg x11grab...");
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
    let mut child = Command::new("ffmpeg")
        .args([
            "-f", "x11grab",
            "-video_size", "1920x1080",
            "-i", &display,
            "-f", "mjpeg",
            "-q:v", "8",
            "-r", "15",
            "-an",
            "pipe:1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("ffmpeg not found, install: sudo apt install ffmpeg")?;

    let stdout = child.stdout.take().unwrap();
    let mut reader = tokio::io::BufReader::with_capacity(256*1024, stdout);
    let mut buf: Vec<u8> = Vec::with_capacity(512*1024);
    let mut chunk = vec![0u8; 65536];

    loop {
        {
            let mgr = manager_arc.lock().await;
            if mgr.role != SessionRole::Target { break; }
        }
        let n = match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 2_000_000 { buf.clear(); }

        // Extract complete JPEG frames
        while let Some(end) = find_jpeg_frame(&buf) {
            let frame = buf[..end].to_vec();
            buf.drain(..end);
            if frame.len() < 1000 { continue; }

            let mut writer_opt = writer_arc.lock().await;
            if let Some(ref mut w) = *writer_opt {
                let total_len = (1 + 4 + frame.len()) as u32;
                let mut hdr = [0u8; 9];
                hdr[..4].copy_from_slice(&total_len.to_be_bytes());
                hdr[4] = relay_client::MSG_FRAME;
                hdr[5..9].copy_from_slice(&(frame.len() as u32).to_be_bytes());
                if w.write_all(&hdr).await.is_err() || w.write_all(&frame).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

/// Find complete JPEG frame (SOI 0xFFD8 ... EOI 0xFFD9), returns position after frame
fn find_jpeg_frame(buf: &[u8]) -> Option<usize> {
    let soi = buf.windows(2).position(|w| w == [0xFF, 0xD8])?;
    let eoi = buf[soi+2..].windows(2).position(|w| w == [0xFF, 0xD9])?;
    Some(soi + 2 + eoi + 2)
}

#[cfg(not(target_os = "linux"))]
async fn start_ffmpeg_pipe(
    _writer_arc: Arc<tokio::sync::Mutex<Option<OwnedWriteHalf>>>,
    _manager_arc: Arc<tokio::sync::Mutex<ConnectionManager>>,
) -> anyhow::Result<()> { Ok(()) }

// ── Common commands ────────────────────────────────────────────

#[tauri::command]
async fn disconnect_session(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.connection_manager.lock().await;
    manager.role = SessionRole::None;
    *state.relay_writer.lock().await = None;
    *state.relay_reader.lock().await = None;
    state.screen_capture.lock().await.stop().map_err(|e| e.to_string())
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
    let data = serde_json::json!({
        "type": event_type,
        "key_code": key_code,
        "x": x,
        "y": y,
        "button": button,
    });
    let payload = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
    let total_len = (1 + 4 + payload.len()) as u32;
    let mut writer_opt = state.relay_writer.lock().await;
    match &mut *writer_opt {
        Some(w) => {
            w.write_all(&total_len.to_be_bytes()).await.map_err(|e| e.to_string())?;
            w.write_all(&[relay_client::MSG_INPUT]).await.map_err(|e| e.to_string())?;
            w.write_all(&(payload.len() as u32).to_be_bytes()).await.map_err(|e| e.to_string())?;
            w.write_all(&payload).await.map_err(|e| e.to_string())?;
            Ok(())
        }
        None => Err("Relay not connected".to_string())
    }
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

#[tauri::command]
async fn simulate_input_event(
    event_type: String,
    key_code: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    button: Option<String>,
) -> Result<(), String> {
    let event = serde_json::json!({
        "type": event_type,
        "key_code": key_code,
        "x": x,
        "y": y,
        "button": button,
    });
    #[cfg(target_os = "linux")]
    simulate_input(&event);
    #[cfg(not(target_os = "linux"))]
    let _ = event;
    Ok(())
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
                relay_writer: Arc::new(Mutex::new(None)),
                relay_reader: Arc::new(Mutex::new(None)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_viewing,
            start_serving,
            disconnect_session,
            send_input_event,
            simulate_input_event,
        ])
        .run(tauri::generate_context!())
        .expect("error while running rem0te client");
}
