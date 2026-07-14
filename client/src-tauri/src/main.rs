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
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let mut mgr = manager_arc.lock().await;
            if mgr.role != SessionRole::Viewer {
                break;
            }
            match mgr.relay.recv().await {
                Ok(data) => {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                    let _ = app.emit("remote-frame", b64);
                }
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
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
                    let _ = mgr.relay.send(&frame).await;
                }
                Err(e) => {
                    log::error!("Capture error: {}", e);
                }
            }
            drop(cap);
            drop(mgr);
            // ~30 FPS target (sleep 33ms between frames)
            tokio::time::sleep(tokio::time::Duration::from_millis(33)).await;
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
    manager.relay.send(&bytes).await.map_err(|e| e.to_string())
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
