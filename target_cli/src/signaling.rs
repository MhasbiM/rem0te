use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

pub struct Signaling {
    write: futures_util::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, tungstenite::Message>,
}

impl Signaling {
    pub async fn connect(server_addr: &str, peer_id: &str) -> Result<(Self, String, String)> {
        let url = format!("ws://{}/", server_addr);
        let (ws, _) = connect_async(&url).await?;
        let (mut write, mut read) = ws.split();

        let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "target".into());
        write.send(tungstenite::Message::Text(serde_json::json!({
            "type": "Register",
            "payload": { "peer_id": peer_id, "os": "linux", "hostname": host }
        }).to_string())).await?;

        let mut relay_host = String::new();
        let mut session_id = String::new();

        while let Some(Ok(msg)) = read.next().await {
            let text = msg.to_text().unwrap_or("");
            let v: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
            match v["type"].as_str() {
                Some("RequestConnection") => {
                    let from = v["payload"]["from_peer"].as_str().unwrap_or("");
                    write.send(tungstenite::Message::Text(serde_json::json!({
                        "type": "ConnectionResponse",
                        "payload": { "from_peer": peer_id, "to_peer": from, "accepted": true }
                    }).to_string())).await?;
                }
                Some("RelayInfo") => {
                    relay_host = v["payload"]["relay_host"].as_str().unwrap_or("").to_string();
                    session_id = v["payload"]["session_id"].as_str().unwrap_or("").to_string();
                    break;
                }
                _ => {}
            }
        }
        if session_id.is_empty() { anyhow::bail!("No RelayInfo received"); }

        // Spawn input listener with remaining read half (write stays alive)
        tokio::spawn(async move {
            while let Some(Ok(msg)) = read.next().await {
                if let Ok(text) = msg.to_text() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
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

        Ok((Self { write }, relay_host, session_id))
    }
}

#[cfg(target_os = "linux")]
fn simulate_input(evt: &serde_json::Value) {
    use std::process::Command;
    match evt["type"].as_str().unwrap_or("") {
        "keyDown" => { if let Some(k) = evt["key_code"].as_str() { let _ = Command::new("xdotool").args(["keydown", &key_name(k)]).output(); } }
        "keyUp"   => { if let Some(k) = evt["key_code"].as_str() { let _ = Command::new("xdotool").args(["keyup", &key_name(k)]).output(); } }
        "mouseMove" => { if let (Some(x), Some(y)) = (evt["x"].as_f64(), evt["y"].as_f64()) { let _ = Command::new("xdotool").args(["mousemove", &format!("{}", x as i32), &format!("{}", y as i32)]).output(); } }
        "mouseDown" => { let b = evt["button"].as_str().unwrap_or("left"); let _ = Command::new("xdotool").args(["mousedown", b]).output(); }
        "mouseUp"   => { let b = evt["button"].as_str().unwrap_or("left"); let _ = Command::new("xdotool").args(["mouseup", b]).output(); }
        _ => {}
    }
}

#[cfg(not(target_os = "linux"))]
fn simulate_input(_: &serde_json::Value) {}

fn key_name(c: &str) -> String {
    let s = match c {
        "Enter" => "Return", "Escape" => "Escape", "Backspace" => "BackSpace",
        "Tab" => "Tab", "Space" => "space",
        "ArrowUp" => "Up", "ArrowDown" => "Down", "ArrowLeft" => "Left", "ArrowRight" => "Right",
        "Shift Left" | "Shift Right" => "Shift_L",
        "Control Left" | "Control Right" => "Control_L",
        "Alt Left" | "Alt Right" => "Alt_L",
        "Meta Left" | "Meta Right" => "Super_L",
        "Delete" => "Delete", "Home" => "Home", "End" => "End",
        "Page Up" => "Prior", "Page Down" => "Next",
        _ => c,
    };
    s.to_string()
}
