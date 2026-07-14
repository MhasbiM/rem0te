//! WebRTC peer connection manager for the remote agent.
//!
//! Handles:
//! - Creating a peer connection with an AV1 video track (proper WebRTC video)
//! - Capturing frames, BGRA→I420 conversion, AV1 encoding via rav1e
//! - Sending encoded AV1 frames through `TrackLocalStaticSample`
//! - Receiving input events via data channel
//! - Exchanging SDP offers/answers via signaling
//!
//! ## Video approach
//! Raw frames are converted to I420 (YUV), encoded with rav1e (AV1, pure Rust),
//! and sent through a proper WebRTC video track. The browser decodes AV1 natively
//! via the `<video>` element's `srcObject`. This replaces the previous JPEG-over-
//! data-channel approach, enabling temporal compression (P-frames) and much
//! higher framerate.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::media::Sample;

use rem0te_shared::SignalingMessage;

use crate::capture::CaptureEngine;
use crate::input::InputEngine;
use crate::video::{self, Av1Encoder};

/// Max width: Full HD.
const MAX_FRAME_WIDTH: u32 = 1920;

/// Target FPS.
const STREAM_FPS: u32 = 25;

/// Target bitrate in kbps.
const BITRATE_KBPS: u32 = 2000;

/// Manages a WebRTC session for one remote viewer.
pub struct WebRtcManager {
    peer_connection: Option<Arc<RTCPeerConnection>>,
    /// AV1 video track for sending encoded frames.
    video_track: Option<Arc<TrackLocalStaticSample>>,
    capture: &'static CaptureEngine,
    input: &'static InputEngine,
    /// Sender to push signaling messages back to the WebSocket.
    signaling_tx: mpsc::UnboundedSender<SignalingMessage>,
    /// Own machine_id for use in signaling messages.
    machine_id: String,
}

impl WebRtcManager {
    pub async fn new(
        capture: &CaptureEngine,
        input: &InputEngine,
        signaling_tx: mpsc::UnboundedSender<SignalingMessage>,
        machine_id: String,
    ) -> Result<Self> {
        // Safety: CaptureEngine and InputEngine are thread-safe (platform APIs
        // use internal synchronization). This transmute lets us share them with
        // spawned tasks without Arc overhead.
        let capture_ref: &'static CaptureEngine = unsafe { std::mem::transmute(capture) };
        let input_ref: &'static InputEngine = unsafe { std::mem::transmute(input) };

        Ok(Self {
            peer_connection: None,
            video_track: None,
            capture: capture_ref,
            input: input_ref,
            signaling_tx,
            machine_id,
        })
    }

    /// Start a new WebRTC session for an incoming connection.
    pub async fn start_session(&mut self, session_id: &str) -> Result<()> {
        info!(session_id = %session_id, "starting WebRTC session");

        // ── Create media engine ───────────────────────────────────
        let mut media_engine = webrtc::api::media_engine::MediaEngine::default();
        media_engine.register_default_codecs()?;

        // ── Create API ────────────────────────────────────────────
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .build();

        // ── ICE servers ───────────────────────────────────────────
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // ── Create peer connection ────────────────────────────────
        let peer_connection = api.new_peer_connection(config).await?;
        let pc = Arc::new(peer_connection);

        // ── AV1 Video Track ──────────────────────────────────────
        // Create a local video track that we'll write encoded AV1 samples to.
        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: webrtc::api::media_engine::MIME_TYPE_AV1.to_owned(),
                clock_rate: 90_000, // 90 kHz for video
                channels: 0,
                sdp_fmtp_line: String::new(),
                rtcp_feedback: vec![],
            },
            "video".to_owned(),
            "rem0te".to_owned(),
        ));

        // Add the track to the peer connection
        let rtp_sender = pc.add_track(video_track.clone()).await?;

        // Spawn a task to read RTCP packets (required by webrtc-rs, otherwise
        // the internal buffer fills up and blocks the connection).
        tokio::spawn(async move {
            let mut buf = vec![0u8; 1500];
            while let Ok((packets, _attrs)) = rtp_sender.read(&mut buf).await {
                debug!("RTCP: {} packets", packets.len());
            }
            debug!("RTCP reader stopped");
        });

        self.video_track = Some(video_track);

        // ── Handle incoming "input" data channel from web client ──
        let input_engine_ref: &'static InputEngine = self.input;
        pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let input = input_engine_ref;
            Box::pin(async move {
                if dc.label() == "input" {
                    info!("incoming 'input' data channel — keyboard/mouse enabled");
                    dc.on_message(Box::new(move |msg| {
                        let inp = input;
                        Box::pin(handle_input_message(msg, inp))
                    }));
                }
            })
        }));

        // ── ICE candidate callback ────────────────────────────────
        let signaling = self.signaling_tx.clone();
        let mid = self.machine_id.clone();
        pc.on_ice_candidate(Box::new(
            move |candidate: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
                let tx = signaling.clone();
                let machine_id = mid.clone();
                Box::pin(async move {
                    if let Some(c) = candidate {
                        if let Ok(cj) = c.to_json() {
                            let msg = SignalingMessage::IceCandidate {
                                from_session: machine_id.clone(),
                                candidate: cj.candidate,
                                sdp_mid: cj.sdp_mid,
                                sdp_m_line_index: cj.sdp_mline_index,
                            };
                            let _ = tx.send(msg);
                        }
                    }
                })
            },
        ));

        // ── Connection state change logging ──────────────────────
        pc.on_peer_connection_state_change(Box::new(move |state| {
            Box::pin(async move {
                info!("peer connection state: {state}");
            })
        }));

        self.peer_connection = Some(pc);

        info!(session_id = %session_id, "WebRTC session ready (AV1 video track)");
        Ok(())
    }

    /// Handle an SDP offer from the web client.
    /// Returns the SDP answer string.
    pub async fn handle_offer(&mut self, sdp: &str) -> Result<String> {
        let pc = self
            .peer_connection
            .as_ref()
            .context("no active peer connection")?;

        let offer =
            webrtc::peer_connection::sdp::session_description::RTCSessionDescription::offer(
                sdp.to_string(),
            )?;
        pc.set_remote_description(offer).await?;

        // Create answer — video track was already added in start_session(),
        // so it will be included in the answer SDP automatically.
        let answer = pc.create_answer(None).await?;
        let answer_sdp = answer.sdp.clone();
        pc.set_local_description(answer).await?;

        info!("SDP answer created with AV1 video track — starting stream");

        // Start the capture → encode → write_sample loop
        self.start_streaming();

        Ok(answer_sdp)
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

    /// Start the capture → encode → video track streaming loop.
    fn start_streaming(&self) {
        let video_track = match self.video_track.clone() {
            Some(t) => t,
            None => {
                error!("no video track to stream to");
                return;
            }
        };
        let capture = self.capture;

        let frame_duration = Duration::from_secs_f64(1.0 / STREAM_FPS as f64);

        tokio::spawn(async move {
            let (display_w, display_h) = capture.display_dimensions();

            // Determine output size (downscale if wider than MAX_FRAME_WIDTH)
            let (enc_w, enc_h) = if display_w > MAX_FRAME_WIDTH {
                let ratio = MAX_FRAME_WIDTH as f64 / display_w as f64;
                (MAX_FRAME_WIDTH, (display_h as f64 * ratio) as u32)
            } else {
                (display_w, display_h)
            };

            // Ensure even dimensions for chroma subsampling
            let enc_w = enc_w.saturating_sub(enc_w % 2);
            let enc_h = enc_h.saturating_sub(enc_h % 2);

            // Create AV1 encoder
            let mut encoder = match Av1Encoder::new(enc_w, enc_h, STREAM_FPS, BITRATE_KBPS) {
                Ok(e) => e,
                Err(e) => {
                    error!("failed to create AV1 encoder: {e}");
                    return;
                }
            };

            let mut last_report = tokio::time::Instant::now();
            let mut frames_since_report = 0u32;
            let mut last_rtp_ts: u32 = 0;
            let rtp_ts_step = 90_000 / STREAM_FPS; // 90kHz clock / FPS

            info!(
                "AV1 video streaming: {}×{} @ {}fps, {}kbps",
                enc_w, enc_h, STREAM_FPS, BITRATE_KBPS
            );

            loop {
                let start = tokio::time::Instant::now();

                match capture_frame_and_encode(capture, &mut encoder) {
                    Ok(Some(encoded)) => {
                        last_rtp_ts = last_rtp_ts.wrapping_add(rtp_ts_step);
                        let encoded_len = encoded.data.len();
                        let encoded_kind = encoded.kind;

                        let sample = Sample {
                            data: bytes::Bytes::from(encoded.data),
                            timestamp: SystemTime::now(),
                            duration: frame_duration,
                            packet_timestamp: last_rtp_ts,
                            prev_dropped_packets: 0,
                            prev_padding_packets: 0,
                        };

                        if let Err(e) = video_track.write_sample(&sample).await {
                            error!("write_sample error: {e}");
                        }

                        frames_since_report += 1;
                        let elapsed = last_report.elapsed();
                        if elapsed.as_secs() >= 3 {
                            let fps = frames_since_report as f64 / elapsed.as_secs_f64();
                            info!(
                                "video: {:.1} FPS, {} bytes/frame, {:?} encode",
                                fps,
                                encoded_len,
                                encoded_kind
                            );
                            last_report = tokio::time::Instant::now();
                            frames_since_report = 0;
                        }
                    }
                    Ok(None) => {
                        // Encoder deferred output (normal during lookahead buildup)
                    }
                    Err(e) => {
                        error!("capture/encode error: {e}");
                    }
                }

                let elapsed = start.elapsed();
                if elapsed < frame_duration {
                    tokio::time::sleep(frame_duration - elapsed).await;
                }
            }
        });
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

// ---------------------------------------------------------------------------
// Capture + Encode Pipeline
// ---------------------------------------------------------------------------

/// Encoded frame with metadata.
struct EncodedFrameData {
    data: Vec<u8>,
    kind: FrameKind,
}

enum FrameKind {
    Key,
    Delta,
}

impl std::fmt::Debug for FrameKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameKind::Key => write!(f, "KEY"),
            FrameKind::Delta => write!(f, "Δ"),
        }
    }
}

/// Capture one frame, convert to I420, encode with AV1.
fn capture_frame_and_encode(
    capture: &CaptureEngine,
    encoder: &mut Av1Encoder,
) -> Result<Option<EncodedFrameData>> {
    let frame = capture.capture_frame()?;

    // BGRA → I420 planes
    let (y, u, v) = video::bgra_to_i420_planes(&frame.data, frame.width, frame.height);

    // Downscale I420 if needed
    let (y, u, v) = if frame.width > MAX_FRAME_WIDTH {
        let ratio = MAX_FRAME_WIDTH as f64 / frame.width as f64;
        let new_w = MAX_FRAME_WIDTH;
        let new_h = (frame.height as f64 * ratio) as u32;
        let new_w = new_w.saturating_sub(new_w % 2);
        let new_h = new_h.saturating_sub(new_h % 2);
        downscale_i420(&y, &u, &v, frame.width, frame.height, new_w, new_h)
    } else {
        let fw = frame.width.saturating_sub(frame.width % 2);
        let fh = frame.height.saturating_sub(frame.height % 2);
        // Truncate to even dimensions if needed (encoder requirement)
        if fw != frame.width || fh != frame.height {
            downscale_i420(&y, &u, &v, frame.width, frame.height, fw, fh)
        } else {
            (y, u, v)
        }
    };

    // Encode with AV1
    match encoder.encode(&y, &u, &v) {
        Ok(Some(encoded)) => {
            let kind = if encoded.keyframe { FrameKind::Key } else { FrameKind::Delta };
            Ok(Some(EncodedFrameData {
                data: encoded.data,
                kind,
            }))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Simple nearest-neighbor downscale for I420 planes.
fn downscale_i420(
    y: &[u8], u: &[u8], v: &[u8],
    src_w: u32, src_h: u32,
    dst_w: u32, dst_h: u32,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let src_w = src_w as usize;
    let src_h = src_h as usize;
    let dst_w = dst_w as usize;
    let dst_h = dst_h as usize;

    let mut dy = vec![0u8; dst_w * dst_h];
    let mut du = vec![0u8; (dst_w / 2) * (dst_h / 2)];
    let mut dv = vec![0u8; (dst_w / 2) * (dst_h / 2)];

    // Downscale Y
    for row in 0..dst_h {
        let src_row = (row * src_h) / dst_h;
        for col in 0..dst_w {
            let src_col = (col * src_w) / dst_w;
            dy[row * dst_w + col] = y[src_row * src_w + src_col];
        }
    }

    // Downscale U and V (half resolution)
    let suw = src_w / 2;
    let suh = src_h / 2;
    let duw = dst_w / 2;
    let duh = dst_h / 2;
    for row in 0..duh {
        let src_row = (row * suh) / duh;
        for col in 0..duw {
            let src_col = (col * suw) / duw;
            du[row * duw + col] = u[src_row * suw + src_col];
            dv[row * duw + col] = v[src_row * suw + src_col];
        }
    }

    (dy, du, dv)
}

// ---------------------------------------------------------------------------
// Data channel: input events from web client
// ---------------------------------------------------------------------------

/// Handle incoming messages on the "input" data channel.
async fn handle_input_message(msg: DataChannelMessage, input: &InputEngine) {
    if msg.is_string {
        if let Ok(text) = String::from_utf8(msg.data.to_vec()) {
            tracing::debug!("input received: {}", &text[..text.len().min(80)]);
            if let Ok(event) = serde_json::from_str::<SignalingMessage>(&text) {
                match event {
                    SignalingMessage::KeyEvent {
                        pressed, key_code, ..
                    } => {
                        let _ = input.send_key_event(key_code, pressed).await;
                    }
                    SignalingMessage::MouseMove { x, y, .. } => {
                        let _ = input.send_mouse_move(x, y).await;
                    }
                    SignalingMessage::MouseButton {
                        button, pressed, ..
                    } => {
                        let _ = input.send_mouse_button(button, pressed).await;
                    }
                    SignalingMessage::MouseScroll { dx, dy, .. } => {
                        let _ = input.send_mouse_scroll(dx, dy).await;
                    }
                    _ => {
                        debug!("unhandled input message variant");
                    }
                }
            }
        }
    } else {
        debug!(
            "received binary input message: {} bytes",
            msg.data.len()
        );
    }
}
