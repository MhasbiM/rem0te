use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

pub async fn wait_for_relay_info(
    server_addr: &str,
    peer_id: &str,
) -> Result<(String, String)> { // returns (relay_host, session_id)
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
                    // Auto-accept
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
    Ok((relay_host, session_id))
}

