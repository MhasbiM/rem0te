//! WebRTC peer connection manager for the remote agent.
//!
//! Handles:
//! - Creating a peer connection with video track
//! - Capturing frames and feeding them to the video track
//! - Receiving input events via data channel
//! - Exchanging SDP offers/answers via signaling
//!
//! ## Implementation notes
//! Uses `webrtc-rs` 0.11 with the media API. The video track is populated
//! with frames captured by the platform-specific `CaptureEngine`.

use std::sync::Arc;

use anyhow::{Context, Result};
use futures_util::SinkExt;
use tokio_tungstenite::tungstenite;
use tracing::{debug, error, info};
use webrtc::api::APIBuilder;
use webrtc::api::media_engine::MIME_TYPE_VP8;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::media::Sample;
use webrtc::data_channel::data_channel_message::DataChannelMessage;

use rem0te_shared::SignalingMessage;

use crate::capture::CaptureEngine;
use crate::input::InputEngine;

/// Manages a WebRTC session for one remote viewer.
pub struct WebRtcManager {
    peer_connection: Option<Arc<RTCPeerConnection>>,
    video_track: Option<Arc<TrackLocalStaticSample>>,
    capture: &'static CaptureEngine,
    input: &'static InputEngine,
}

// Safety: CaptureEngine and InputEngine are thread-safe (they use platform
// APIs that are internally synchronized).
// Actually, this is a simplified approach. In production, use Arc.
impl WebRtcManager {
    pub async fn new(
        capture: &CaptureEngine,
        input: &InputEngine,
    ) -> Result<Self> {
        // Note: In production, we'd use Arc<CaptureEngine> and Arc<InputEngine>
        // but for the boilerplate we transmute the references to 'static.
        // This is safe as long as the engines outlive the manager.
        let capture_ref: &'static CaptureEngine = unsafe { std::mem::transmute(capture) };
        let input_ref: &'static InputEngine = unsafe { std::mem::transmute(input) };

        Ok(Self {
            peer_connection: None,
            video_track: None,
            capture: capture_ref,
            input: input_ref,
        })
    }

    /// Start a new WebRTC session for an incoming connection.
    pub async fn start_session(
        &mut self,
        session_id: &str,
        _ws_tx: &mut (impl SinkExt<tungstenite::Message, Error = impl std::error::Error + Send + Sync> + Unpin),
        _machine_id: &str,
    ) -> Result<()> {
        info!(session_id = %session_id, "starting WebRTC session");

        // ── Create media engine ───────────────────────────────────
        let mut media_engine = webrtc::api::media_engine::MediaEngine::default();

        // Register video codec (VP8 for broad compatibility, H.264 for hardware)
        media_engine.register_default_codecs()?;

        // ── Create API ────────────────────────────────────────────
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .build();

        // ── ICE servers (STUN/TURN) ───────────────────────────────
        let config = RTCConfiguration {
            ice_servers: vec![
                RTCIceServer {
                    urls: vec!["stun:stun.l.google.com:19302".to_string()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        // ── Create peer connection ────────────────────────────────
        let peer_connection = api.new_peer_connection(config).await?;

        // ── Create video track ────────────────────────────────────
        let video_track = Arc::new(TrackLocalStaticSample::new(
            webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
                mime_type: MIME_TYPE_VP8.to_string(),
                ..Default::default()
            },
            format!("video-{session_id}"),
            format!("stream-{session_id}"),
        ));

        // Add the track to the peer connection
        let rtp_sender = peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        // Read RTCP packets (required by the spec)
        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        });

        // ── Data channel for input events ─────────────────────────
        let data_channel = peer_connection.create_data_channel("input", None).await?;

        let input_engine_ref: &'static InputEngine = self.input; // same transmute approach
        data_channel.on_message(Box::new(move |msg| {
            let input = input_engine_ref;
            Box::pin(handle_data_channel_message(msg, input))
        }));

        // ── ICE candidate callback ────────────────────────────────
        // Need to clone ws_tx sender or use Arc<Mutex<...>>
        // For now, we store state and send candidates during the negotiation

        self.peer_connection = Some(Arc::new(peer_connection));
        self.video_track = Some(video_track);

        // TODO: Create SDP offer and send to web client via signaling
        // The web client creates the offer (browser initiates), so the agent
        // waits for the offer, then creates an answer.

        info!(session_id = %session_id, "WebRTC session ready");

        Ok(())
    }

    /// Handle an SDP offer from the web client.
    pub async fn handle_offer(&mut self, sdp: &str) -> Result<()> {
        let pc = self
            .peer_connection
            .as_ref()
            .context("no active peer connection")?;

        // Parse and set remote description
        let offer = webrtc::peer_connection::sdp::session_description::RTCSessionDescription::offer(sdp.to_string())?;
        pc.set_remote_description(offer).await?;

        // Create answer
        let answer = pc.create_answer(None).await?;
        pc.set_local_description(answer.clone()).await?;

        // TODO: Send answer back via signaling
        info!("SDP answer created, ready to send to web client");

        // Start streaming frames
        self.start_streaming().await?;

        Ok(())
    }

    /// Handle an ICE candidate from the web client.
    pub async fn handle_ice_candidate(
        &mut self,
        candidate: &str,
        sdp_mid: Option<&str>,
        sdp_mline_index: Option<u16>,
    ) -> Result<()> {
        let pc = self
            .peer_connection
            .as_ref()
            .context("no active peer connection")?;

        let ice = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
            candidate: candidate.to_string(),
            sdp_mid: sdp_mid.map(|s| s.to_string()),
            sdp_mline_index,
            username_fragment: None,
        };

        pc.add_ice_candidate(ice).await?;

        debug!("ICE candidate added");
        Ok(())
    }

    /// Start capturing and streaming frames to the video track.
    async fn start_streaming(&self) -> Result<()> {
        let track = self
            .video_track
            .clone()
            .context("no video track")?;
        let capture = self.capture; // &'static reference

        tokio::spawn(async move {
            let fps = 30;
            let frame_duration = std::time::Duration::from_secs_f64(1.0 / fps as f64);

            loop {
                let start = tokio::time::Instant::now();

                match capture.capture_frame() {
                    Ok(frame) => {
                        let sample = Sample {
                            data: bytes::Bytes::from(frame.data),
                            duration: frame_duration,
                            ..Default::default()
                        };

                        if let Err(e) = track.write_sample(&sample).await {
                            error!("failed to write video sample: {e}");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("frame capture error: {e}");
                    }
                }

                let elapsed = start.elapsed();
                if elapsed < frame_duration {
                    tokio::time::sleep(frame_duration - elapsed).await;
                }
            }
        });

        info!("video streaming started at 30fps");
        Ok(())
    }

    /// Close the WebRTC session.
    pub async fn close(&mut self) -> Result<()> {
        if let Some(pc) = self.peer_connection.take() {
            if let Err(e) = pc.close().await {
                error!("error closing peer connection: {e}");
            }
        }
        self.video_track = None;
        info!("WebRTC session closed");
        Ok(())
    }
}

/// Handle incoming messages on the WebRTC data channel.
///
/// Messages are input events forwarded from the web client.
async fn handle_data_channel_message(
    msg: DataChannelMessage,
    input: &InputEngine,
) {
    if msg.is_string {
        if let Ok(text) = String::from_utf8(msg.data.to_vec()) {
            // Parse as signaling message (input events)
            if let Ok(event) = serde_json::from_str::<SignalingMessage>(&text) {
                match event {
                    SignalingMessage::KeyEvent { pressed, key_code, .. } => {
                        let _ = input.send_key_event(key_code, pressed).await;
                    }
                    SignalingMessage::MouseMove { x, y, .. } => {
                        let _ = input.send_mouse_move(x, y).await;
                    }
                    SignalingMessage::MouseButton { button, pressed, .. } => {
                        let _ = input.send_mouse_button(button, pressed).await;
                    }
                    SignalingMessage::MouseScroll { dx, dy, .. } => {
                        let _ = input.send_mouse_scroll(dx, dy).await;
                    }
                    _ => {
                        debug!("unhandled data channel message: {text}");
                    }
                }
            }
        }
    } else {
        debug!("received binary data channel message: {} bytes", msg.data.len());
    }
}
