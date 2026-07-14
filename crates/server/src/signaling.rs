//! Signaling hub — the central message broker.
//!
//! Maintains two connection pools:
//! 1. **Agents** — remote machines that can be controlled (keyed by `MachineId`).
//! 2. **Web clients** — browsers that want to control a remote machine (keyed by `SessionId`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::ws::Message;
use dashmap::DashMap;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use rem0te_shared::{MachineId, MachineInfo, SessionId, SignalingMessage};

// ---------------------------------------------------------------------------
// Hub
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SignalingHub {
    inner: Arc<HubInner>,
}

struct HubInner {
    /// Connected remote agents: MachineId → agent handle.
    agents: DashMap<MachineId, AgentHandle>,
    /// Connected web clients: SessionId → client handle.
    web_clients: DashMap<SessionId, WebClientHandle>,
    /// Active connections: MachineId → web client SessionId.
    active_connections: DashMap<MachineId, SessionId>,
    /// Auth token required for agent registration.
    token: String,
}

/// Sender for pushing messages to a specific WebSocket connection.
pub type WsTx = mpsc::UnboundedSender<Message>;

struct AgentHandle {
    session_id: SessionId,
    info: MachineInfo,
    tx: WsTx,
    missed_heartbeats: AtomicU64,
}

// Manual Clone — AtomicU64 doesn't implement Clone
impl Clone for AgentHandle {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            info: self.info.clone(),
            tx: self.tx.clone(),
            missed_heartbeats: AtomicU64::new(self.missed_heartbeats.load(Ordering::SeqCst)),
        }
    }
}

#[derive(Debug, Clone)]
struct WebClientHandle {
    tx: WsTx,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

impl SignalingHub {
    /// Create a new hub.
    pub fn new(token: String) -> Self {
        Self {
            inner: Arc::new(HubInner {
                agents: DashMap::new(),
                web_clients: DashMap::new(),
                active_connections: DashMap::new(),
                token,
            }),
        }
    }

    // ── Agent operations ────────────────────────────────────────────

    /// Register (or re-register) a remote agent.
    ///
    /// Returns the receiver channel the agent should listen on for incoming
    /// messages from the server.
    pub fn register_agent(
        &self,
        machine_id: MachineId,
        machine_name: String,
        os: String,
        os_version: String,
        display_width: u32,
        display_height: u32,
        token: &str,
        tx: WsTx,
    ) -> Result<(SessionId, mpsc::UnboundedReceiver<Message>), String> {
        // Validate token
        if token != self.inner.token {
            return Err("invalid token".into());
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let info = MachineInfo {
            machine_id: machine_id.clone(),
            machine_name,
            os,
            os_version,
            display_width,
            display_height,
            online: true,
        };

        let handle = AgentHandle {
            session_id: session_id.clone(),
            info: info.clone(),
            tx: tx.clone(),
            missed_heartbeats: AtomicU64::new(0),
        };

        let was_new = self.inner.agents.insert(machine_id.clone(), handle).is_none();
        if was_new {
            info!(machine_id = %machine_id, "agent registered");
        } else {
            info!(machine_id = %machine_id, "agent re-registered (reconnected)");
        }

        // Notify all web clients about the new/updated machine
        self.broadcast_to_web_clients(SignalingMessage::MachineOnline { machine: info });

        // Create a receiver channel for this agent
        let (_agent_tx, agent_rx) = mpsc::unbounded_channel();
        // We already have the sender stored — actually, the agent reads from
        // its own WebSocket directly. The `tx` above is for pushing messages
        // TO the agent. The agent needs a way to receive messages FROM the
        // hub. We use the same `tx` — when hub wants to send to agent, it
        // calls `tx.send(...)`.

        Ok((session_id, agent_rx))
    }

    /// Unregister an agent (called on disconnect).
    pub fn unregister_agent(&self, machine_id: &MachineId) {
        if self.inner.agents.remove(machine_id).is_some() {
            info!(machine_id = %machine_id, "agent unregistered");
            // Clean up any active connection
            self.inner.active_connections.remove(machine_id);
            self.broadcast_to_web_clients(SignalingMessage::MachineOffline {
                machine_id: machine_id.clone(),
            });
        }
    }

    /// Reset heartbeat counter for an agent.
    pub fn agent_heartbeat(&self, machine_id: &MachineId) {
        if let Some(handle) = self.inner.agents.get(machine_id) {
            handle
                .missed_heartbeats
                .store(0, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Increment missed heartbeats; return true if agent should be kicked.
    pub fn agent_missed_heartbeat(&self, machine_id: &MachineId, max_missed: u64) -> bool {
        if let Some(handle) = self.inner.agents.get(machine_id) {
            let missed = handle
                .missed_heartbeats
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            missed >= max_missed
        } else {
            false
        }
    }

    /// Check if an agent is online.
    pub fn is_agent_online(&self, machine_id: &MachineId) -> bool {
        self.inner.agents.contains_key(machine_id)
    }

    /// Send a message to a specific agent.
    pub fn send_to_agent(&self, machine_id: &MachineId, msg: SignalingMessage) -> bool {
        if let Some(handle) = self.inner.agents.get(machine_id) {
            let text = serde_json::to_string(&msg).unwrap_or_default();
            let _ = handle.tx.send(Message::Text(text.into()));
            true
        } else {
            warn!(machine_id = %machine_id, "send_to_agent: agent not found");
            false
        }
    }

    /// Get the WebSocket sender for an agent.
    pub fn agent_tx(&self, machine_id: &MachineId) -> Option<WsTx> {
        self.inner
            .agents
            .get(machine_id)
            .map(|h| h.tx.clone())
    }

    /// Get the session id for an agent.
    pub fn agent_session_id(&self, machine_id: &MachineId) -> Option<SessionId> {
        self.inner
            .agents
            .get(machine_id)
            .map(|h| h.session_id.clone())
    }

    // ── Web client operations ───────────────────────────────────────

    /// Register a web client connection.
    pub fn register_web_client(&self, tx: WsTx) -> SessionId {
        let session_id = uuid::Uuid::new_v4().to_string();
        let handle = WebClientHandle { tx };
        self.inner
            .web_clients
            .insert(session_id.clone(), handle);
        debug!(session_id = %session_id, "web client connected");
        session_id
    }

    /// Unregister a web client.
    pub fn unregister_web_client(&self, session_id: &SessionId) {
        self.inner.web_clients.remove(session_id);
        debug!(session_id = %session_id, "web client disconnected");
    }

    /// Send a message to a specific web client.
    pub fn send_to_web_client(&self, session_id: &SessionId, msg: SignalingMessage) -> bool {
        if let Some(handle) = self.inner.web_clients.get(session_id) {
            let text = serde_json::to_string(&msg).unwrap_or_default();
            let _ = handle.tx.send(Message::Text(text.into()));
            true
        } else {
            warn!(session_id = %session_id, "send_to_web_client: not found");
            false
        }
    }

    /// Send a message to all connected web clients.
    pub fn broadcast_to_web_clients(&self, msg: SignalingMessage) {
        let text = serde_json::to_string(&msg).unwrap_or_default();
        let message = Message::Text(text.into());
        for entry in self.inner.web_clients.iter() {
            let _ = entry.tx.send(message.clone());
        }
    }

    /// Get currently online machines.
    pub fn list_machines(&self) -> Vec<MachineInfo> {
        self.inner
            .agents
            .iter()
            .map(|entry| entry.info.clone())
            .collect()
    }

    /// Look up which machine a web client is connected to.
    pub fn get_machine_for_web_client(&self, web_session_id: &SessionId) -> Option<MachineId> {
        self.inner
            .active_connections
            .iter()
            .find(|entry| entry.value() == web_session_id)
            .map(|entry| entry.key().clone())
    }

    /// Record that a web client is now connected to a machine.
    pub fn set_active_connection(&self, machine_id: MachineId, web_session_id: SessionId) {
        info!(%machine_id, %web_session_id, "active connection established");
        self.inner
            .active_connections
            .insert(machine_id, web_session_id);
    }

    /// Remove the active connection for a machine.
    pub fn remove_active_connection(&self, machine_id: &MachineId) {
        self.inner.active_connections.remove(machine_id);
    }

    /// Get the web client session connected to a machine.
    pub fn get_web_client_for_machine(&self, machine_id: &MachineId) -> Option<SessionId> {
        self.inner
            .active_connections
            .get(machine_id)
            .map(|entry| entry.value().clone())
    }
}
