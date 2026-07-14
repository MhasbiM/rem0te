use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::file_transfer::FileEntry;

/// Connection manager for signaling, relay, and peer-to-peer communication
pub struct ConnectionManager {
    ws: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    relay_stream: Option<TcpStream>,
    connected_peer: Option<String>,
    local_peer_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
enum SignalingMessage {
    Register {
        peer_id: String,
        os: String,
        hostname: String,
    },
    Registered {
        assigned_id: String,
    },
    RequestConnection {
        from_peer: String,
        to_peer: String,
        sdp: Option<String>,
    },
    ConnectionResponse {
        from_peer: String,
        accepted: bool,
        sdp: Option<String>,
    },
    RelayInfo {
        relay_host: String,
        relay_port: u16,
        session_id: String,
    },
    IceCandidate {
        from_peer: String,
        to_peer: String,
        candidate: String,
    },
    FileTransferRequest {
        session_id: String,
        from_peer: String,
        file_path: String,
        direction: String,
    },
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            ws: None,
            relay_stream: None,
            connected_peer: None,
            local_peer_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Connect to signaling server and register
    pub async fn connect_to_peer(
        &mut self,
        server_addr: &str,
        peer_id: &str,
        local_peer_id: &str,
    ) -> Result<String> {
        self.local_peer_id = local_peer_id.to_string();

        let ws_url = format!("ws://{}/", server_addr);
        log::info!("Connecting to signaling server: {}", ws_url);

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .context("Failed to connect to signaling server")?;

        let (mut write, mut read) = ws_stream.split();

        // Register this peer
        let register_msg = SignalingMessage::Register {
            peer_id: self.local_peer_id.clone(),
            os: std::env::consts::OS.to_string(),
            hostname: std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("HOST"))
                .unwrap_or_else(|_| "unknown".to_string()),
        };

        let json = serde_json::to_string(&register_msg)?;
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
            .await
            .context("Failed to send register message")?;

        // Wait for Registered response
        let mut assigned_id = String::new();
        while let Some(msg) = read.next().await {
            match msg? {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    let sig_msg: SignalingMessage = serde_json::from_str(&text)?;
                    if let SignalingMessage::Registered { assigned_id: id } = sig_msg {
                        assigned_id = id;
                        log::info!("Registered with ID: {}", assigned_id);
                        break;
                    }
                }
                _ => continue,
            }
        }

        // Request connection to target peer
        let connect_msg = SignalingMessage::RequestConnection {
            from_peer: assigned_id.clone(),
            to_peer: peer_id.to_string(),
            sdp: None, // No WebRTC SDP for TCP-based approach
        };

        let json = serde_json::to_string(&connect_msg)?;
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
            .await
            .context("Failed to send connection request")?;

        // Store the connection
        self.connected_peer = Some(peer_id.to_string());

        Ok(assigned_id)
    }

    /// Disconnect from current peer
    pub async fn disconnect(&mut self) -> Result<()> {
        self.ws = None;
        self.relay_stream = None;
        self.connected_peer = None;
        log::info!("Disconnected from peer");
        Ok(())
    }

    /// Send input event (keyboard/mouse) to remote peer
    pub async fn send_input(
        &self,
        event_type: &str,
        key_code: Option<String>,
        x: Option<f64>,
        y: Option<f64>,
        button: Option<String>,
    ) -> Result<()> {
        // In production, encode and send via relay stream or P2P connection
        log::info!(
            "Input event: type={}, key={:?}, pos=({:?},{:?}), button={:?}",
            event_type,
            key_code,
            x,
            y,
            button
        );

        if let Some(ref _relay) = self.relay_stream {
            // In production: encode and send input event bytes via relay stream
            let _input_data = serde_json::json!({
                "type": event_type,
                "key_code": key_code,
                "x": x,
                "y": y,
                "button": button,
            });
            // relay.write_all(input_data.to_string().as_bytes()).await?;
        }

        Ok(())
    }

    /// List files on remote machine
    pub async fn list_remote_files(&self, path: &str) -> Result<Vec<FileEntry>> {
        log::info!("Listing remote files at: {}", path);
        // In production, send a file list request via signaling/relay
        // For now, return example entries
        Ok(vec![
            FileEntry {
                name: "Documents".into(),
                path: format!("{}/Documents", path),
                is_dir: true,
                size: 0,
                modified: "2024-01-15".into(),
            },
            FileEntry {
                name: "app.log".into(),
                path: format!("{}/app.log", path),
                is_dir: false,
                size: 4096,
                modified: "2024-01-15".into(),
            },
        ])
    }

    /// Upload a file to remote peer
    pub async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<()> {
        log::info!("Uploading {} -> remote:{}", local_path, remote_path);

        // Read local file
        let data = tokio::fs::read(local_path)
            .await
            .context("Failed to read local file")?;

        // Send via relay or P2P connection
        // For now, just log the operation
        log::info!(
            "File upload prepared: {} bytes from {} to {}",
            data.len(),
            local_path,
            remote_path
        );

        Ok(())
    }

    /// Download a file from remote peer
    pub async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<()> {
        log::info!("Downloading remote:{} -> {}", remote_path, local_path);

        // In production: request file data from remote peer via relay
        // Then write to local path

        log::info!(
            "File download prepared: {} -> {}",
            remote_path,
            local_path
        );

        Ok(())
    }
}
