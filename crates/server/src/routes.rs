//! HTTP routes: health check, WebSocket upgrade, and static file serving.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tracing::{debug, info, warn};

use rem0te_shared::{MachineId, SessionId, SignalingMessage};

use crate::signaling::SignalingHub;

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub hub: SignalingHub,
    pub heartbeat_secs: u64,
    pub heartbeat_missed: u64,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn build_router(state: AppState, web_dir: Option<String>) -> Router {
    let mut router = Router::new()
        .route("/health", get(health_check))
        .route("/ws", get(ws_upgrade))
        .with_state(state);

    // Optionally serve the Vue frontend
    if let Some(dir) = web_dir {
        info!("serving static files from: {dir}");
        router = router.fallback_service(
            tower_http::services::ServeDir::new(dir),
        );
    }

    router
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health_check() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "rem0te-server",
    }))
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!(%addr, "WebSocket upgrade request");
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

async fn handle_socket(socket: WebSocket, addr: SocketAddr, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Channel for pushing messages to this socket from the hub
    let (hub_tx, mut hub_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    // We don't know yet if this is an agent or a web client.
    // The first message will tell us.
    let mut role: Option<Role> = None;

    // ── Write task ────────────────────────────────────────────────
    let write_handle = tokio::spawn(async move {
        while let Some(msg) = hub_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // ── Read task ─────────────────────────────────────────────────
    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Text(text) => {
                let parsed: Result<SignalingMessage, _> = serde_json::from_str(&text);
                match parsed {
                    Ok(sig_msg) => {
                        handle_signaling_message(
                            sig_msg,
                            &mut role,
                            &addr,
                            &state,
                            &hub_tx,
                        )
                        .await;
                    }
                    Err(e) => {
                        warn!(%addr, "invalid message: {e}");
                        let _ = hub_tx.send(Message::Text(
                            serde_json::to_string(&SignalingMessage::error(
                                "bad_request",
                                format!("invalid JSON: {e}"),
                            ))
                            .unwrap()
                            .into(),
                        ));
                    }
                }
            }
            Message::Close(_) => {
                info!(%addr, "websocket closed by peer");
                break;
            }
            Message::Ping(data) => {
                let _ = hub_tx.send(Message::Pong(data));
            }
            _ => {}
        }
    }

    // ── Cleanup ───────────────────────────────────────────────────
    write_handle.abort();

    match role {
        Some(Role::Agent(machine_id)) => {
            state.hub.unregister_agent(&machine_id);
        }
        Some(Role::WebClient(session_id)) => {
            state.hub.unregister_web_client(&session_id);
        }
        None => {}
    }

    info!(%addr, "websocket disconnected");
}

// ---------------------------------------------------------------------------
// Message dispatcher
// ---------------------------------------------------------------------------

enum Role {
    Agent(MachineId),
    WebClient(SessionId),
}

#[allow(clippy::too_many_arguments)]
async fn handle_signaling_message(
    msg: SignalingMessage,
    role: &mut Option<Role>,
    addr: &SocketAddr,
    state: &AppState,
    hub_tx: &tokio::sync::mpsc::UnboundedSender<Message>,
    // TODO: pass the hub's agent_tx for forwarding
    // We'll use state.hub for now
) {
    match &msg {
        // ── Agent messages ─────────────────────────────────────
        SignalingMessage::Register {
            machine_id,
            machine_name,
            os,
            os_version,
            display_width,
            display_height,
            token,
        } => {
            // Use the hub_tx as the sender for this agent
            let tx = hub_tx.clone();
            match state.hub.register_agent(
                machine_id.clone(),
                machine_name.clone(),
                os.clone(),
                os_version.clone(),
                *display_width,
                *display_height,
                token,
                tx,
            ) {
                Ok((session_id, _rx)) => {
                    *role = Some(Role::Agent(machine_id.clone()));

                    // Send registration confirmation
                    let reply = SignalingMessage::Registered {
                        session_id: session_id.clone(),
                    };
                    let _ = hub_tx.send(Message::Text(
                        serde_json::to_string(&reply).unwrap().into(),
                    ));

                    info!(%addr, machine_id = %machine_id, "agent registered successfully");
                }
                Err(e) => {
                    warn!(%addr, machine_id = %machine_id, "agent registration failed: {e}");
                    let reply = SignalingMessage::error("auth_failed", e);
                    let _ = hub_tx.send(Message::Text(
                        serde_json::to_string(&reply).unwrap().into(),
                    ));
                }
            }
        }

        SignalingMessage::Heartbeat => {
            if let Some(Role::Agent(machine_id)) = role {
                state.hub.agent_heartbeat(machine_id);
            }
        }

        // ── Web Client messages ─────────────────────────────────
        SignalingMessage::ListMachines => {
            // First-time web clients get registered here
            if role.is_none() {
                let session_id = state.hub.register_web_client(hub_tx.clone());
                *role = Some(Role::WebClient(session_id));
            }

            let machines = state.hub.list_machines();
            let reply = SignalingMessage::MachineList { machines };
            let _ = hub_tx.send(Message::Text(
                serde_json::to_string(&reply).unwrap().into(),
            ));
        }

        SignalingMessage::ConnectToMachine { machine_id } => {
            // Register web client if not already
            if role.is_none() {
                let session_id = state.hub.register_web_client(hub_tx.clone());
                *role = Some(Role::WebClient(session_id));
            }

            if state.hub.is_agent_online(machine_id) {
                // Tell the agent about the incoming connection
                if let Some(web_session_id) = role.as_ref().and_then(|r| match r {
                    Role::WebClient(sid) => Some(sid.clone()),
                    _ => None,
                }) {
                    // Track the connection: machine → web client
                    state.hub.set_active_connection(machine_id.clone(), web_session_id.clone());

                    let agent_msg = SignalingMessage::IncomingConnection {
                        session_id: uuid::Uuid::new_v4().to_string(),
                        web_client_id: web_session_id,
                    };
                    state.hub.send_to_agent(machine_id, agent_msg);

                    let reply = SignalingMessage::Connected {
                        machine_id: machine_id.clone(),
                        session_id: uuid::Uuid::new_v4().to_string(),
                    };
                    let _ = hub_tx.send(Message::Text(
                        serde_json::to_string(&reply).unwrap().into(),
                    ));
                }
            } else {
                let reply = SignalingMessage::ConnectionFailed {
                    machine_id: machine_id.clone(),
                    reason: "machine offline".into(),
                };
                let _ = hub_tx.send(Message::Text(
                    serde_json::to_string(&reply).unwrap().into(),
                ));
            }
        }

        // ── WebRTC relay (bidirectional) ────────────────────────
        // WebRtcAnswer is used by BOTH:
        //   - Web client: sends SDP offer TO a machine
        //   - Agent: sends SDP answer back TO server (to forward to web client)
        SignalingMessage::WebRtcAnswer { target_machine, sdp } => {
            match role {
                Some(Role::WebClient(_)) => {
                    // Web client → Agent: forward the offer
                    let relay = SignalingMessage::WebRtcOffer {
                        from_session: "web".into(),
                        sdp: sdp.clone(),
                    };
                    state.hub.send_to_agent(target_machine, relay);
                }
                Some(Role::Agent(_)) => {
                    // Agent → Web Client: forward the answer
                    if let Some(web_session_id) = state.hub.get_web_client_for_machine(target_machine) {
                        let relay = SignalingMessage::WebRtcAnswerFromAgent {
                            machine_id: target_machine.clone(),
                            sdp: sdp.clone(),
                        };
                        state.hub.send_to_web_client(&web_session_id, relay);
                        debug!("relayed SDP answer from agent → web client");
                    } else {
                        warn!(machine_id = %target_machine, "no web client connected to this machine");
                    }
                }
                None => {
                    warn!("WebRtcAnswer from unidentified peer");
                }
            }
        }

        SignalingMessage::IceCandidateToAgent {
            target_machine,
            candidate,
            sdp_mid,
            sdp_m_line_index,
        } => {
            let relay = SignalingMessage::IceCandidate {
                from_session: "web".into(),
                candidate: candidate.clone(),
                sdp_mid: sdp_mid.clone(),
                sdp_m_line_index: *sdp_m_line_index,
            };
            state.hub.send_to_agent(target_machine, relay);
        }

        // ── WebRTC relay (Agent → Web Client ICE) ─────────────────
        SignalingMessage::IceCandidate {
            from_session: _,
            candidate,
            sdp_mid,
            sdp_m_line_index,
        } => {
            // from_session is the agent identifying itself; find the connected machine
            if let Some(Role::Agent(ref machine_id)) = role {
                if let Some(web_session_id) = state.hub.get_web_client_for_machine(machine_id) {
                    let relay = SignalingMessage::IceCandidateFromAgent {
                        machine_id: machine_id.clone(),
                        candidate: candidate.clone(),
                        sdp_mid: sdp_mid.clone(),
                        sdp_m_line_index: *sdp_m_line_index,
                    };
                    state.hub.send_to_web_client(&web_session_id, relay);
                    debug!("relayed ICE candidate from agent → web client");
                }
            }
        }

        // ── Input events ────────────────────────────────────────
        SignalingMessage::KeyEvent { target, .. }
        | SignalingMessage::MouseMove { target, .. }
        | SignalingMessage::MouseButton { target, .. }
        | SignalingMessage::MouseScroll { target, .. } => {
            // Forward directly to the target agent
            let _ = state.hub.send_to_agent(target, msg.clone());
        }

        // ── Disconnect ──────────────────────────────────────────
        SignalingMessage::Disconnect => {
            // Could trigger cleanup of WebRTC session
        }

        // Everything else — these are server→client messages, ignore
        _ => {
            debug!(%addr, ?msg, "ignored message from client");
        }
    }
}
