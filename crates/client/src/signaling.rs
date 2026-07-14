//! WebSocket signaling client for the remote agent.
//!
//! Connects to the signaling server, registers this machine, and maintains
//! the connection. Handles reconnection and heartbeats automatically.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tracing::{debug, error, info, warn};

use rem0te_shared::SignalingMessage;

use crate::config::Config;
use crate::capture::CaptureEngine;
use crate::input::InputEngine;
use crate::webrtc::WebRtcManager;

/// Runs the signaling client loop. This is the main entry point for the
/// remote agent — it connects to the server and handles all signaling.
pub async fn run(config: Config) -> anyhow::Result<()> {
    let machine_id = config
        .machine_id
        .clone()
        .unwrap_or_else(|| hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string()));

    let machine_name = config
        .name
        .clone()
        .unwrap_or_else(|| machine_id.clone());

    // Platform-specific engines
    let capture = CaptureEngine::new().await?;
    let input = InputEngine::new().await?;

    let (display_width, display_height) = capture.display_dimensions();

    info!(
        machine_id = %machine_id,
        machine_name = %machine_name,
        display = format!("{display_width}x{display_height}"),
        "remote agent starting"
    );

    loop {
        match connect_and_run(
            &config,
            &machine_id,
            &machine_name,
            display_width,
            display_height,
            &capture,
            &input,
        )
        .await
        {
            Ok(()) => {
                info!("signaling loop ended, reconnecting...");
            }
            Err(e) => {
                error!("signaling error: {e}, reconnecting in {}s...", config.reconnect_secs);
            }
        }

        tokio::time::sleep(Duration::from_secs(config.reconnect_secs)).await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn connect_and_run(
    config: &Config,
    machine_id: &str,
    machine_name: &str,
    display_width: u32,
    display_height: u32,
    capture: &CaptureEngine,
    input: &InputEngine,
) -> anyhow::Result<()> {
    // Connect to signaling server
    info!("connecting to signaling server: {}", config.server);
    let (ws_stream, _response) = connect_async(&config.server).await?;
    info!("connected to signaling server");

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Send registration
    let register_msg = SignalingMessage::Register {
        machine_id: machine_id.to_string(),
        machine_name: machine_name.to_string(),
        os: std::env::consts::OS.to_string(),
        os_version: os_version(),
        display_width,
        display_height,
        token: config.token.clone(),
    };

    let json = serde_json::to_string(&register_msg)?;
    ws_tx
        .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
        .await?;

    // Heartbeat ticker
    let heartbeat_interval = config.heartbeat_secs;
    let mut heartbeat_tick = tokio::time::interval(Duration::from_secs(heartbeat_interval));

    // Channel for WebRTC manager to push messages back to this loop
    let (webrtc_tx, mut webrtc_rx) = tokio::sync::mpsc::unbounded_channel::<SignalingMessage>();

    // WebRTC manager (lazy-initialized when connection requested)
    let mut webrtc_manager: Option<WebRtcManager> = None;

    // Message loop
    loop {
        tokio::select! {
            // ── Incoming messages from server ─────────────────────
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        let parsed: Result<SignalingMessage, _> = serde_json::from_str(&text);
                        match parsed {
                            Ok(sig_msg) => {
                                handle_server_message(
                                    sig_msg,
                                    &mut webrtc_manager,
                                    &mut ws_tx,
                                    machine_id,
                                    capture,
                                    input,
                                    &webrtc_tx,
                                )
                                .await?;
                            }
                            Err(e) => {
                                warn!("failed to parse server message: {e}");
                            }
                        }
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                        info!("server closed connection");
                        break;
                    }
                    Some(Err(e)) => {
                        error!("websocket error: {e}");
                        break;
                    }
                    None => {
                        info!("websocket stream ended");
                        break;
                    }
                    _ => {}
                }
            }

            // ── Heartbeat ─────────────────────────────────────────
            _ = heartbeat_tick.tick() => {
                let hb = SignalingMessage::Heartbeat;
                let json = serde_json::to_string(&hb).unwrap();
                if ws_tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
                    .await
                    .is_err()
                {
                    warn!("failed to send heartbeat");
                    break;
                }
            }

            // ── Outgoing WebRTC signaling ─────────────────────────
            Some(out_msg) = webrtc_rx.recv() => {
                let json = serde_json::to_string(&out_msg).unwrap();
                if ws_tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
                    .await
                    .is_err()
                {
                    warn!("failed to send WebRTC signaling message");
                    break;
                }
                debug!("sent WebRTC signaling: {:?}", out_msg);
            }
        }
    }

    Ok(())
}

/// Handle a message received from the signaling server.
#[allow(clippy::too_many_arguments)]
async fn handle_server_message(
    msg: SignalingMessage,
    webrtc_manager: &mut Option<WebRtcManager>,
    ws_tx: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::Message,
    >,
    machine_id: &str,
    capture: &CaptureEngine,
    input: &InputEngine,
    webrtc_tx: &tokio::sync::mpsc::UnboundedSender<SignalingMessage>,
) -> anyhow::Result<()> {
    match msg {
        SignalingMessage::Registered { session_id } => {
            info!(session_id = %session_id, "registered with server");
        }

        SignalingMessage::IncomingConnection {
            session_id,
            web_client_id: _,
        } => {
            info!(
                session_id = %session_id,
                "incoming connection request"
            );

            // Initialize WebRTC with the signaling channel
            let mut wm = WebRtcManager::new(
                capture,
                input,
                webrtc_tx.clone(),
                machine_id.to_string(),
            )
            .await?;
            wm.start_session(&session_id).await?;
            *webrtc_manager = Some(wm);
        }

        SignalingMessage::WebRtcOffer {
            from_session: _,
            sdp,
        } => {
            if let Some(ref mut wm) = webrtc_manager {
                debug!("received WebRTC offer from web client");
                let answer_sdp = wm.handle_offer(&sdp).await?;
                // Send answer back to server
                let reply = SignalingMessage::WebRtcAnswer {
                    target_machine: machine_id.to_string(),
                    sdp: answer_sdp,
                };
                let json = serde_json::to_string(&reply)?;
                ws_tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
                    .await?;
                info!("sent SDP answer back to signaling server");
            }
        }

        SignalingMessage::IceCandidate {
            from_session: _,
            candidate,
            sdp_mid,
            sdp_m_line_index,
        } => {
            if let Some(ref mut wm) = webrtc_manager {
                debug!("received ICE candidate");
                wm.handle_ice_candidate(&candidate, sdp_mid.as_deref(), sdp_m_line_index)
                    .await?;
            }
        }

        SignalingMessage::PeerDisconnected { session_id: _ } => {
            info!("peer disconnected, cleaning up WebRTC session");
            *webrtc_manager = None;
        }

        // Input events from web client
        SignalingMessage::KeyEvent {
            pressed,
            key_code,
            ..
        } => {
            input.send_key_event(key_code, pressed).await?;
        }
        SignalingMessage::MouseMove { x, y, .. } => {
            input.send_mouse_move(x, y).await?;
        }
        SignalingMessage::MouseButton {
            button, pressed, ..
        } => {
            input.send_mouse_button(button, pressed).await?;
        }
        SignalingMessage::MouseScroll { dx, dy, .. } => {
            input.send_mouse_scroll(dx, dy).await?;
        }

        other => {
            debug!("unhandled message: {other:?}");
        }
    }

    Ok(())
}

/// Get a human-readable OS version string.
fn os_version() -> String {
    #[cfg(target_os = "linux")]
    {
        // Try to read from /etc/os-release
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "Linux".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        // Use sw_vers or uname
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| format!("macOS {}", s.trim()))
            .unwrap_or_else(|| "macOS".to_string())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        format!("{} (unknown)", std::env::consts::OS)
    }
}
