use dashmap::DashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

/// A relay session between two peers
#[derive(Debug, Clone)]
pub struct RelaySession {
    pub session_id: String,
    pub peer_a: String,
    pub peer_b: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct RelayState {
    pub sessions: DashMap<String, RelaySession>,
    // session_id -> (peer_id, tx)
    pub channels: DashMap<String, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>,
}

impl RelayState {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            channels: DashMap::new(),
        }
    }

    pub fn create_session(&self, peer_id: &str) -> String {
        let session_id = Uuid::new_v4().to_string();
        self.sessions.insert(session_id.clone(), RelaySession {
            session_id: session_id.clone(),
            peer_a: peer_id.to_string(),
            peer_b: None,
            created_at: chrono::Utc::now(),
        });
        session_id
    }

    pub fn join_session(&self, session_id: &str, peer_id: &str) -> bool {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.peer_b = Some(peer_id.to_string());
            true
        } else {
            false
        }
    }
}

/// Run the TCP relay server for proxying data between peers
pub async fn run_relay_server(state: Arc<RelayState>, port: u16) -> anyhow::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    log::info!("Relay server listening on port {}", port);

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_relay_connection(state, stream, addr).await {
                log::error!("Relay connection error from {}: {}", addr, e);
            }
        });
    }
}

async fn handle_relay_connection(
    state: Arc<RelayState>,
    mut stream: TcpStream,
    addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    // First 36 bytes = session_id (UUID)
    let mut session_buf = [0u8; 36];
    stream.read_exact(&mut session_buf).await?;
    let session_id = String::from_utf8_lossy(&session_buf).to_string();

    // Next byte indicates role: 0 = peer_a (initiator), 1 = peer_b (joiner)
    let mut role_buf = [0u8; 1];
    stream.read_exact(&mut role_buf).await?;

    let channel_id = format!("{}-{}", session_id, role_buf[0]);

    // Create a channel for this peer to receive relayed data
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    state.channels.insert(channel_id.clone(), tx);
    let peer_channel_id = format!("{}-{}", session_id, 1 - role_buf[0]);

    // Read from this peer and forward to the other
    let write_state = state.clone();
    let write_channel = peer_channel_id.clone();
    let (mut read_half, mut write_half) = stream.into_split();

    // Forward from this peer -> other peer
    let forward_handle = tokio::spawn(async move {
        let mut buf = [0u8; 65536];
        loop {
            match read_half.read(&mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if let Some(target_tx) = write_state.channels.get(&write_channel) {
                        let _ = target_tx.send(buf[..n].to_vec());
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Forward from other peer -> this peer
    let recv_handle = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if write_half.write_all(&data).await.is_err() {
                break;
            }
        }
    });

    forward_handle.await?;
    recv_handle.abort();
    state.channels.remove(&channel_id);

    Ok(())
}
