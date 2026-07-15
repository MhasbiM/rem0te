use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;

pub async fn wait_for_relay_info(
    server_addr: &str,
    peer_id: &str,
) -> Result<(String, String)> {
    let url = format!("ws://{}/", server_addr);
    log::info!("Target connecting to signaling: {}", url);
    let (ws, _) = connect_async(&url).await.context("WS connect")?;
    let (mut write, mut read) = ws.split();

    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "target".into());
    let msg = serde_json::json!({
        "type": "Register",
        "payload": { "peer_id": peer_id, "os": "linux", "hostname": host }
    });
    write.send(tungstenite::Message::Text(msg.to_string())).await?;

    let mut relay_host = String::new();
    let mut session_id = String::new();

    while let Some(Ok(msg)) = read.next().await {
        if let tungstenite::Message::Text(text) = msg {
            let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            match v["type"].as_str() {
                Some("RequestConnection") => {
                    let from = v["payload"]["from_peer"].as_str().unwrap_or("");
                    let resp = serde_json::json!({
                        "type": "ConnectionResponse",
                        "payload": { "from_peer": peer_id, "to_peer": from, "accepted": true, "sdp": null }
                    });
                    write.send(tungstenite::Message::Text(resp.to_string())).await?;
                    log::info!("Accepted connection from {}", from);
                }
                Some("RelayInfo") => {
                    relay_host = v["payload"]["relay_host"].as_str().unwrap_or("").to_string();
                    session_id = v["payload"]["session_id"].as_str().unwrap_or("").to_string();
                    log::info!("Got relay info: {} session={}", relay_host, session_id);
                    break;
                }
                _ => {}
            }
        }
    }
    if session_id.is_empty() { anyhow::bail!("Never received RelayInfo"); }

    // Spawn input listener with remaining read half
    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            if let tungstenite::Message::Text(text) = msg {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v["type"] == "InputEvent" {
                        if let Some(evt_str) = v["payload"]["event"].as_str() {
                            if let Ok(evt) = serde_json::from_str::<serde_json::Value>(evt_str) {
                                simulate_input(&evt);
                            }
                        }
                    }
                }
            }
        }
    });

    Ok((relay_host, session_id))
}

#[cfg(target_os = "linux")]
fn simulate_input(evt: &serde_json::Value) {
    use std::process::Command;
    let etype = evt["type"].as_str().unwrap_or("");
    match etype {
        "keyDown" => { if let Some(k) = evt["key_code"].as_str() { let _ = Command::new("xdotool").args(["keydown", &key_to_xdotool(k)]).output(); } }
        "keyUp"   => { if let Some(k) = evt["key_code"].as_str() { let _ = Command::new("xdotool").args(["keyup", &key_to_xdotool(k)]).output(); } }
        "mouseMove" => { if let (Some(x), Some(y)) = (evt["x"].as_f64(), evt["y"].as_f64()) { let _ = Command::new("xdotool").args(["mousemove", &format!("{}", x as i32), &format!("{}", y as i32)]).output(); } }
        "mouseDown" => { let b = evt["button"].as_str().unwrap_or("left"); let _ = Command::new("xdotool").args(["mousedown", b]).output(); }
        "mouseUp"   => { let b = evt["button"].as_str().unwrap_or("left"); let _ = Command::new("xdotool").args(["mouseup", b]).output(); }
        _ => {}
    }
}

#[cfg(not(target_os = "linux"))]
fn simulate_input(_evt: &serde_json::Value) {}

fn key_to_xdotool(code: &str) -> String {
    match code {
        "Enter" => "Return", "Escape" => "Escape", "Backspace" => "BackSpace",
        "Tab" => "Tab", "Space" => "space",
        "ArrowUp" => "Up", "ArrowDown" => "Down", "ArrowLeft" => "Left", "ArrowRight" => "Right",
        "ShiftLeft" | "ShiftRight" => "Shift_L",
        "ControlLeft" | "ControlRight" => "Control_L",
        "AltLeft" | "AltRight" => "Alt_L",
        "MetaLeft" | "MetaRight" => "Super_L",
        "Delete" => "Delete", "Home" => "Home", "End" => "End",
        "PageUp" => "Prior", "PageDown" => "Next",
        s if s.starts_with("Key") => &s[3..],
        s if s.starts_with("Digit") => &s[5..],
        _ => code,
    }.to_lowercase()
}

