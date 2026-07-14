use anyhow::{Context, Result};
use crate::relay_client::RelayClient;
use crate::file_transfer::FileEntry;

/// Manages remote desktop session state and relay data channel.
/// Signaling is handled by React frontend via WebSocket; this manages the data plane.
pub struct ConnectionManager {
    pub relay: RelayClient,
    pub connected_peer: Option<String>,
    pub local_peer_id: String,
    pub server_addr: String,
    pub role: SessionRole,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionRole {
    None,
    Viewer,   // The one viewing/controlling the remote desktop
    Target,   // The one being viewed/controlled
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            relay: RelayClient::new(),
            connected_peer: None,
            local_peer_id: String::new(),
            server_addr: String::new(),
            role: SessionRole::None,
        }
    }

    /// Viewer (Mac): connect to relay & create session
    pub async fn start_viewing(&mut self, server_addr: &str, peer_id: &str) -> Result<String> {
        self.server_addr = server_addr.to_string();
        self.connected_peer = Some(peer_id.to_string());
        self.role = SessionRole::Viewer;
        let relay_addr = relay_host_from(server_addr);
        let sid = self.relay.create_session(&relay_addr).await?;
        log::info!("Viewer relay session {} created", sid);
        Ok(sid)
    }

    /// Target (Linux): join relay session
    pub async fn start_serving(&mut self, server_addr: &str, session_id: &str, viewer_peer: &str) -> Result<()> {
        self.server_addr = server_addr.to_string();
        self.connected_peer = Some(viewer_peer.to_string());
        self.role = SessionRole::Target;
        let relay_addr = relay_host_from(server_addr);
        self.relay.join_session(&relay_addr, session_id).await?;
        log::info!("Target joined relay session {}", session_id);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.relay.disconnect();
        self.connected_peer = None;
        self.role = SessionRole::None;
    }
}

fn relay_host_from(server_addr: &str) -> String {
    let addr = server_addr.trim_end_matches('/');
    if let Some(pos) = addr.rfind(':') {
        format!("{}:21117", &addr[..pos])
    } else {
        format!("{}:21117", addr)
    }
}
