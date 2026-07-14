// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod connection;
mod file_transfer;

use capture::ScreenCapture;
use connection::ConnectionManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub connection_manager: Arc<Mutex<ConnectionManager>>,
    pub screen_capture: Arc<Mutex<ScreenCapture>>,
}

#[tauri::command]
async fn connect_to_peer(
    state: tauri::State<'_, AppState>,
    server_addr: String,
    peer_id: String,
    local_peer_id: String,
) -> Result<String, String> {
    let mut manager = state.connection_manager.lock().await;
    manager
        .connect_to_peer(&server_addr, &peer_id, &local_peer_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn disconnect_peer(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.connection_manager.lock().await;
    manager.disconnect().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_capture(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut capture = state.screen_capture.lock().await;
    capture.start().map_err(|e| e.to_string())
}

#[tauri::command]
async fn stop_capture(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut capture = state.screen_capture.lock().await;
    capture.stop().map_err(|e| e.to_string())
}

use base64::Engine;

#[tauri::command]
async fn capture_frame(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let mut capture = state.screen_capture.lock().await;
    let frame_data = capture.capture_frame().map_err(|e| e.to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&frame_data);
    Ok(encoded)
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
    let manager = state.connection_manager.lock().await;
    manager
        .send_input(&event_type, key_code, x, y, button)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_remote_files(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<Vec<file_transfer::FileEntry>, String> {
    let manager = state.connection_manager.lock().await;
    manager
        .list_remote_files(&path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn upload_file(
    state: tauri::State<'_, AppState>,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    let manager = state.connection_manager.lock().await;
    manager
        .upload_file(&local_path, &remote_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_file(
    state: tauri::State<'_, AppState>,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    let manager = state.connection_manager.lock().await;
    manager
        .download_file(&remote_path, &local_path)
        .await
        .map_err(|e| e.to_string())
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
            connect_to_peer,
            disconnect_peer,
            start_capture,
            stop_capture,
            capture_frame,
            send_input_event,
            list_remote_files,
            upload_file,
            download_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running rem0te client");
}
