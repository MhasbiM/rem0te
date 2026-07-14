//! Shared types and protocol definitions for rem0te.
//!
//! This crate defines the signaling protocol messages exchanged between:
//! - Remote agent (client) ↔ Signaling server
//! - Web browser (frontend) ↔ Signaling server

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Unique identifier for a machine (hostname-derived or UUID).
pub type MachineId = String;

/// Unique identifier for a signaling session (WebSocket connection).
pub type SessionId = String;

// ---------------------------------------------------------------------------
// Machine info
// ---------------------------------------------------------------------------

/// Information about a remote machine available for connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInfo {
    pub machine_id: MachineId,
    pub machine_name: String,
    pub os: String,
    pub os_version: String,
    pub display_width: u32,
    pub display_height: u32,
    pub online: bool,
}

// ---------------------------------------------------------------------------
// Signaling messages
// ---------------------------------------------------------------------------

/// All messages exchanged over the signaling WebSocket.
///
/// Uses serde's internally-tagged enum representation so that
/// `{"type": "register", ...}` deserializes to the correct variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalingMessage {
    // ── Agent → Server ──────────────────────────────────────────────

    /// Register this machine as available for remote control.
    Register {
        machine_id: MachineId,
        machine_name: String,
        os: String,
        os_version: String,
        display_width: u32,
        display_height: u32,
        /// Auth token shared with the server.
        token: String,
    },

    /// Periodic keep-alive.
    Heartbeat,

    // ── Server → Agent ──────────────────────────────────────────────

    /// Registration accepted.
    Registered {
        session_id: SessionId,
    },

    /// Another peer wants to connect.
    IncomingConnection {
        session_id: SessionId,
        web_client_id: SessionId,
    },

    /// Forwarded WebRTC SDP offer from the web client.
    WebRtcOffer {
        from_session: SessionId,
        sdp: String,
    },

    /// Forwarded ICE candidate from the web client.
    IceCandidate {
        from_session: SessionId,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },

    /// The web client disconnected.
    PeerDisconnected {
        session_id: SessionId,
    },

    // ── Web Client → Server ─────────────────────────────────────────

    /// Request the list of online machines.
    ListMachines,

    /// Request to connect to a specific machine.
    ConnectToMachine {
        machine_id: MachineId,
    },

    /// Disconnect from the current remote session.
    Disconnect,

    /// Forward WebRTC SDP answer (or offer) to the agent.
    WebRtcAnswer {
        target_machine: MachineId,
        sdp: String,
    },

    /// Forward ICE candidate to the agent.
    IceCandidateToAgent {
        target_machine: MachineId,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },

    // ── Server → Web Client ─────────────────────────────────────────

    /// Response with available machines.
    MachineList {
        machines: Vec<MachineInfo>,
    },

    /// Successfully connected to a remote machine.
    Connected {
        machine_id: MachineId,
        session_id: SessionId,
    },

    /// Connection attempt failed.
    ConnectionFailed {
        machine_id: MachineId,
        reason: String,
    },

    /// A machine came online.
    MachineOnline {
        machine: MachineInfo,
    },

    /// A machine went offline.
    MachineOffline {
        machine_id: MachineId,
    },

    /// Forwarded WebRTC SDP answer from the agent.
    WebRtcAnswerFromAgent {
        machine_id: MachineId,
        sdp: String,
    },

    /// Forwarded ICE candidate from the agent.
    IceCandidateFromAgent {
        machine_id: MachineId,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },

    // ── Input events (Web Client → Server → Agent) ──────────────────

    KeyEvent {
        target: MachineId,
        pressed: bool,
        key_code: u16,
    },

    MouseMove {
        target: MachineId,
        x: f64,
        y: f64,
    },

    MouseButton {
        target: MachineId,
        button: u8,
        pressed: bool,
    },

    MouseScroll {
        target: MachineId,
        dx: f64,
        dy: f64,
    },

    // ── Errors ──────────────────────────────────────────────────────

    Error {
        code: String,
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Helper constructors
// ---------------------------------------------------------------------------

impl SignalingMessage {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_roundtrip() {
        let msg = SignalingMessage::Register {
            machine_id: "test-machine".into(),
            machine_name: "Test PC".into(),
            os: "linux".into(),
            os_version: "Ubuntu 24.04".into(),
            display_width: 1920,
            display_height: 1080,
            token: "secret".into(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: SignalingMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            SignalingMessage::Register { machine_id, .. } => {
                assert_eq!(machine_id, "test-machine");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_tag_field() {
        let json = r#"{"type":"heartbeat"}"#;
        let msg: SignalingMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, SignalingMessage::Heartbeat));
    }
}
