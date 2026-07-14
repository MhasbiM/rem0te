use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

/// A connected peer in the signaling system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    pub peer_id: String,
    pub os: String,
    pub hostname: String,
    pub online: bool,
    pub addr: Option<String>,
    #[serde(skip)]
    pub ws_sender: Option<Arc<tokio::sync::mpsc::UnboundedSender<String>>>,
}

/// Signaling message exchanged between peers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SignalingMessage {
    Register {
        peer_id: String,
        os: String,
        hostname: String,
    },
    Registered {
        assigned_id: String,
    },
    PeerList {
        peers: Vec<PeerInfo>,
    },
    RequestConnection {
        from_peer: String,
        to_peer: String,
        sdp: Option<String>,
    },
    ConnectionResponse {
        from_peer: String,
        to_peer: String,
        accepted: bool,
        sdp: Option<String>,
    },
    IceCandidate {
        from_peer: String,
        to_peer: String,
        candidate: String,
    },
    RelayInfo {
        relay_host: String,
        relay_port: u16,
        session_id: String,
        to_peer: String,
    },
    PeerOnline {
        peer: PeerInfo,
    },
    PeerOffline {
        peer_id: String,
    },
    SessionEnd {
        from_peer: String,
        to_peer: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub os: String,
    pub hostname: String,
    pub online: bool,
}

pub struct SignalingState {
    pub peers: DashMap<String, Peer>,
    // peer_id -> (tx, addr)
    pub tcp_connections: DashMap<String, (Arc<tokio::sync::mpsc::UnboundedSender<String>>, SocketAddr)>,
    // ws peer_id -> tx
    pub ws_connections: DashMap<String, tokio::sync::mpsc::UnboundedSender<String>>,
}

impl SignalingState {
    pub fn new() -> Self {
        Self {
            peers: DashMap::new(),
            tcp_connections: DashMap::new(),
            ws_connections: DashMap::new(),
        }
    }

    pub fn register_peer(&self, peer_id: String, os: String, hostname: String, addr: SocketAddr) -> String {
        let id = Uuid::new_v4().to_string();
        self.peers.insert(id.clone(), Peer {
            id: id.clone(),
            peer_id,
            os,
            hostname,
            online: true,
            addr: Some(addr.to_string()),
            ws_sender: None,
        });
        id
    }

    pub fn get_peer_list(&self) -> Vec<PeerInfo> {
        self.peers.iter().map(|p| PeerInfo {
            peer_id: p.peer_id.clone(),
            os: p.os.clone(),
            hostname: p.hostname.clone(),
            online: p.online,
        }).collect()
    }

    pub fn set_offline(&self, id: &str) {
        if let Some(mut peer) = self.peers.get_mut(id) {
            peer.online = false;
        }
    }
}

/// Run the TCP-based signaling server (for native clients)
pub async fn run_tcp_server(state: Arc<SignalingState>, port: u16) -> anyhow::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    log::info!("TCP Signaling server listening on port {}", port);

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_tcp_connection(state, stream, addr).await {
                log::error!("TCP signaling error from {}: {}", addr, e);
            }
        });
    }
}

async fn handle_tcp_connection(
    state: Arc<SignalingState>,
    stream: TcpStream,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    // First message must be Register
    buf_reader.read_line(&mut line).await?;
    let msg: SignalingMessage = serde_json::from_str(line.trim())?;

    let peer_id = if let SignalingMessage::Register { peer_id, os, hostname } = msg {
        let id = state.register_peer(peer_id.clone(), os, hostname, addr);
        let response = SignalingMessage::Registered { assigned_id: id.clone() };
        writer.write_all(serde_json::to_string(&response)?.as_bytes()).await?;
        writer.write_all(b"\n").await?;

        // Notify others
        if let Some(peer) = state.peers.get(&id) {
            broadcast_peer_online(&state, &peer);
        }
        id
    } else {
        return Err(anyhow::anyhow!("First message must be Register"));
    };

    // Create channel for this connection
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let tx = Arc::new(tx);
    state.tcp_connections.insert(peer_id.clone(), (tx.clone(), addr));

    // Read loop
    let read_state = state.clone();
    let read_peer_id = peer_id.clone();
    let read_handle = tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            // We need a new reader... this is simplified
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    // Write loop - forward messages to this peer
    let write_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            // write to stream would need proper handling
            let _ = msg;
        }
    });

    // On disconnect
    state.set_offline(&peer_id);
    state.tcp_connections.remove(&peer_id);
    read_handle.abort();
    write_handle.abort();

    Ok(())
}

fn broadcast_peer_online(state: &SignalingState, peer: &Peer) {
    let msg = SignalingMessage::PeerOnline {
        peer: PeerInfo {
            peer_id: peer.peer_id.clone(),
            os: peer.os.clone(),
            hostname: peer.hostname.clone(),
            online: true,
        },
    };
    let json = serde_json::to_string(&msg).unwrap();
    for conn in state.tcp_connections.iter() {
        let _ = conn.value().0.send(json.clone());
    }
    for ws in state.ws_connections.iter() {
        let _ = ws.value().send(json.clone());
    }
}

/// Run WebSocket signaling server (for web-based clients)
pub async fn run_ws_server(state: Arc<SignalingState>, port: u16) -> anyhow::Result<()> {
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    use futures_util::StreamExt;

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    log::info!("WebSocket Signaling server listening on port {}", port);

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            match accept_async(stream).await {
                Ok(ws_stream) => {
                    let (mut ws_sender_ws, mut ws_receiver) = ws_stream.split();
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                    let peer_id = Uuid::new_v4().to_string();
                    state.ws_connections.insert(peer_id.clone(), tx.clone());

                    // Send outgoing messages to WS
                    let send_handle = tokio::spawn(async move {
                        use futures_util::SinkExt;
                        while let Some(msg) = rx.recv().await {
                            let _ = ws_sender_ws
                                .send(tokio_tungstenite::tungstenite::Message::Text(msg.into()))
                                .await;
                        }
                    });

                    // Read incoming from WS
                    while let Some(Ok(msg)) = ws_receiver.next().await {
                        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                            if let Ok(sig_msg) = serde_json::from_str::<SignalingMessage>(&text) {
                                log::info!("WS recv from {}: {:?}", peer_id, sig_msg);
                                handle_signaling_message(&state, &peer_id, sig_msg);
                            } else {
                                log::warn!("WS parse fail from {}: {}", peer_id, &text[..text.len().min(200)]);
                            }
                        }
                    }

                    state.ws_connections.remove(&peer_id);
                    state.set_offline(&peer_id);
                    send_handle.abort();
                }
                Err(e) => {
                    log::error!("WS accept error from {}: {}", addr, e);
                }
            }
        });
    }
}

fn handle_signaling_message(state: &SignalingState, sender_id: &str, msg: SignalingMessage) {
    match &msg {
        SignalingMessage::Register { peer_id, os, hostname } => {
            state.peers.insert(sender_id.to_string(), Peer {
                id: sender_id.to_string(),
                peer_id: peer_id.clone(),
                os: os.clone(),
                hostname: hostname.clone(),
                online: true,
                addr: None,
                ws_sender: None,
            });
            let response = SignalingMessage::Registered { assigned_id: sender_id.to_string() };
            let json = serde_json::to_string(&response).unwrap();
            if let Some(tx) = state.ws_connections.get(sender_id) {
                let _ = tx.send(json);
            }
            // Send peer list to the newly registered peer
            let list = SignalingMessage::PeerList { peers: state.get_peer_list() };
            let json = serde_json::to_string(&list).unwrap();
            if let Some(tx) = state.ws_connections.get(sender_id) {
                let _ = tx.send(json);
            }
            // Notify all peers about the new peer
            let new_peer = PeerInfo {
                peer_id: peer_id.clone(),
                os: os.clone(),
                hostname: hostname.clone(),
                online: true,
            };
            let online_msg = SignalingMessage::PeerOnline { peer: new_peer };
            let online_json = serde_json::to_string(&online_msg).unwrap();
            for conn in state.ws_connections.iter() {
                if conn.key() != sender_id {
                    let _ = conn.value().send(online_json.clone());
                }
            }
        }
        SignalingMessage::RequestConnection { to_peer, .. } => {
            // Resolve peer_id -> internal connection ID
            let target_id = state.peers.iter()
                .find(|p| p.peer_id == *to_peer && p.online)
                .map(|p| p.id.clone());

            log::info!("RequestConnection: to_peer={}, resolved={:?}", to_peer, target_id);

            let json = serde_json::to_string(&msg).unwrap();
            if let Some(ref id) = target_id {
                if let Some(target) = state.ws_connections.get(id) {
                    log::info!("Forwarding to WS {}", id);
                    let _ = target.send(json);
                    return;
                } else if let Some(target) = state.tcp_connections.get(id) {
                    let _ = target.0.send(json);
                    return;
                }
            }
            // Target not found - send error back to requestor
            let error_msg = SignalingMessage::Error {
                message: format!("Peer '{}' not found or offline", to_peer),
            };
            let error_json = serde_json::to_string(&error_msg).unwrap();
            if let Some(tx) = state.ws_connections.get(sender_id) {
                let _ = tx.send(error_json);
            }
        }
        SignalingMessage::ConnectionResponse { ref to_peer, .. } => {
            // Route response to the intended recipient (to_peer)
            let target_id = state.peers.iter()
                .find(|p| p.peer_id == *to_peer && p.online)
                .map(|p| p.id.clone());

            let json = serde_json::to_string(&msg).unwrap();
            if let Some(ref id) = target_id {
                if let Some(target) = state.ws_connections.get(id) {
                    let _ = target.send(json);
                } else if let Some(target) = state.tcp_connections.get(id) {
                    let _ = target.0.send(json);
                }
            }
        }
        SignalingMessage::IceCandidate { ref to_peer, .. } => {
            let target_id = state.peers.iter()
                .find(|p| p.peer_id == *to_peer && p.online)
                .map(|p| p.id.clone());
            let json = serde_json::to_string(&msg).unwrap();
            if let Some(ref id) = target_id {
                if let Some(target) = state.ws_connections.get(id) {
                    let _ = target.send(json);
                } else if let Some(target) = state.tcp_connections.get(id) {
                    let _ = target.0.send(json);
                }
            }
        }
        SignalingMessage::RelayInfo { ref to_peer, .. } => {
            let target_id = state.peers.iter()
                .find(|p| p.peer_id == *to_peer && p.online)
                .map(|p| p.id.clone());
            log::info!("RelayInfo: to_peer={}, resolved={:?}", to_peer, target_id);
            let json = serde_json::to_string(&msg).unwrap();
            if let Some(ref id) = target_id {
                if let Some(target) = state.ws_connections.get(id) {
                    let _ = target.send(json);
                }
            }
        }
        SignalingMessage::SessionEnd { ref to_peer, .. } => {
            let target_id = state.peers.iter()
                .find(|p| p.peer_id == *to_peer && p.online)
                .map(|p| p.id.clone());
            log::info!("SessionEnd: to_peer={}, resolved={:?}", to_peer, target_id);
            let json = serde_json::to_string(&msg).unwrap();
            if let Some(ref id) = target_id {
                if let Some(target) = state.ws_connections.get(id) {
                    let _ = target.send(json);
                }
            }
        }
        _ => {
            log::warn!("Unhandled signaling message: {:?}", msg);
        }
    }
}
