//! WebRTC peer connection manager for the remote agent.
//!
//! Handles:
//! - Creating a peer connection
//! - Capturing frames, encoding to JPEG, and sending via data channel
//! - Receiving input events via data channel
//! - Exchanging SDP offers/answers via signaling
//!
//! ## Video approach
//! Raw frames are encoded to JPEG and sent over a "video" data channel.
//! The frontend renders them as `<img>` elements. This avoids the complexity
//! of real-time video encoding (VP8/H.264) while still delivering a
//! functional remote desktop stream.
//!
//! Future: replace JPEG with hardware-accelerated H.264 via VideoToolbox
//! (macOS) or VAAPI (Linux) piped through a proper WebRTC video track.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;

use rem0te_shared::SignalingMessage;

use crate::capture::CaptureEngine;
use crate::input::InputEngine;

/// Max chunk size for data channel (must be < 64KB SCTP limit).
const CHUNK_SIZE: usize = 60_000;

/// JPEG quality. Lower = faster encoding + smaller chunks.
const JPEG_QUALITY: u8 = 55;

/// Max width: 1280 for faster X11 capture (~60ms vs ~125ms at 1920).
const MAX_FRAME_WIDTH: u32 = 1280;

/// Target FPS.
const STREAM_FPS: u32 = 10;

/// Manages a WebRTC session for one remote viewer.
pub struct WebRtcManager {
    peer_connection: Option<Arc<RTCPeerConnection>>,
    /// Data channel for sending JPEG video frames.
    video_dc: Option<Arc<RTCDataChannel>>,
    capture: &'static CaptureEngine,
    input: &'static InputEngine,
    /// Sender to push signaling messages back to the WebSocket.
    signaling_tx: mpsc::UnboundedSender<SignalingMessage>,
    /// Own machine_id for use in signaling messages.
    machine_id: String,
}

// Safety: CaptureEngine and InputEngine are thread-safe (they use platform
// APIs that are internally synchronized).
// Actually, this is a simplified approach. In production, use Arc.
impl WebRtcManager {
    pub async fn new(
        capture: &CaptureEngine,
        input: &InputEngine,
        signaling_tx: mpsc::UnboundedSender<SignalingMessage>,
        machine_id: String,
    ) -> Result<Self> {
        let capture_ref: &'static CaptureEngine = unsafe { std::mem::transmute(capture) };
        let input_ref: &'static InputEngine = unsafe { std::mem::transmute(input) };

        Ok(Self {
            peer_connection: None,
            video_dc: None,
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

        // ── Data channel for video frames (JPEG) ──────────────────
        // Agent creates "video" channel → web client receives via ondatachannel
        let video_dc = peer_connection
            .create_data_channel("video", None)
            .await?;
        self.video_dc = Some(video_dc);

        // ── Handle incoming "input" channel from web client ────────
        // Web client creates "input" channel → agent receives via on_data_channel
        let input_engine_ref: &'static InputEngine = self.input;
        peer_connection.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let input = input_engine_ref;
            Box::pin(async move {
                if dc.label() == "input" {
                    tracing::info!("incoming 'input' data channel — keyboard/mouse enabled");
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
        peer_connection.on_ice_candidate(Box::new(
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

        self.peer_connection = Some(Arc::new(peer_connection));

        info!(session_id = %session_id, "WebRTC session ready");
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

        // Create answer
        let answer = pc.create_answer(None).await?;
        let answer_sdp = answer.sdp.clone();
        pc.set_local_description(answer).await?;

        info!("SDP answer created — starting video stream");

        // Start streaming JPEG frames (spawns background task)
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

    /// Start capturing, encoding, chunking, and sending frames.
    fn start_streaming(&self) {
        let video_dc = match self.video_dc.clone() {
            Some(dc) => dc,
            None => {
                error!("no video data channel");
                return;
            }
        };
        let capture = self.capture;

        let frame_duration = Duration::from_secs_f64(1.0 / STREAM_FPS as f64);

        tokio::spawn(async move {
            let mut frame_id: u32 = 0;
            info!("video streaming (chunked JPEG, {}fps, q{})", STREAM_FPS, JPEG_QUALITY);

            loop {
                let start = tokio::time::Instant::now();

                match capture_frame_jpeg(capture) {
                    Ok(jpeg_bytes) => {
                        frame_id = frame_id.wrapping_add(1);
                        let chunks = split_into_chunks(frame_id, &jpeg_bytes);

                        for chunk in chunks {
                            if video_dc.send(&bytes::Bytes::from(chunk)).await.is_err() {
                                error!("failed to send chunk");
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        error!("frame error: {e}");
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
        self.video_dc = None;
        info!("WebRTC session closed");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Frame encoding + cursor
// ---------------------------------------------------------------------------

/// Capture a frame, draw cursor, downscale, encode to JPEG.
fn capture_frame_jpeg(capture: &CaptureEngine) -> Result<Vec<u8>> {
    let frame = capture.capture_frame()?;

    // Determine output size
    let (out_w, out_h) = if frame.width > MAX_FRAME_WIDTH {
        let ratio = MAX_FRAME_WIDTH as f64 / frame.width as f64;
        (MAX_FRAME_WIDTH, (frame.height as f64 * ratio) as u32)
    } else {
        (frame.width, frame.height)
    };

    // BGRA → RGB (drop alpha, swap R/B)
    let pixel_count = (frame.width * frame.height) as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    for chunk in frame.data.chunks_exact(4) {
        rgb.push(chunk[2]); // R
        rgb.push(chunk[1]); // G
        rgb.push(chunk[0]); // B
    }

    // Cursor is rendered by frontend (CSS overlay), not drawn on frame.
    // This avoids JPEG artifacts on the cursor and keeps it sharp.

    let src_img = image::RgbImage::from_raw(frame.width, frame.height, rgb)
        .context("failed to create source image")?;

    // Downscale
    let img = if out_w != frame.width {
        image::imageops::resize(&src_img, out_w, out_h, image::imageops::FilterType::Nearest)
    } else {
        src_img
    };

    // JPEG encode
    let mut jpeg_bytes = Vec::new();
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, JPEG_QUALITY);
    encoder.encode(&img, out_w, out_h, image::ColorType::Rgb8.into())?;

    debug!("frame {}x{}→{}x{} JPEG {} bytes", frame.width, frame.height, out_w, out_h, jpeg_bytes.len());
    Ok(jpeg_bytes)
}

/// Draw a clean arrow cursor using geometric lines.
/// White arrow with 1px black outline, 16x24 pixels.
fn draw_cursor(rgb: &mut [u8], width: u32, height: u32, cx: u32, cy: u32) {
    let white = [255u8, 255, 255];
    let black = [0u8, 0, 0];

    // Helper: set pixel if in bounds
    let mut set = |px: i32, py: i32, color: &[u8; 3]| {
        if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
            let idx = ((py as u32) * width + (px as u32)) as usize * 3;
            if idx + 2 < rgb.len() {
                rgb[idx] = color[0]; rgb[idx + 1] = color[1]; rgb[idx + 2] = color[2];
            }
        }
    };

    let cx = cx as i32;
    let cy = cy as i32;

    // Outline: draw black 1px border around the arrow shape
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            // Diagonal edge (top-left to middle-right)
            for i in 0..=14 { set(cx + i + dx, cy + i + dy, &black); }
            // Vertical stem
            for i in 5..=22 { set(cx + 7 + dx, cy + i + dy, &black); }
            // Horizontal bar at bottom
            for i in 0..=7 { set(cx + i + dx, cy + 18 + dy, &black); }
        }
    }

    // Fill: draw white interior
    for i in 1..=13 { set(cx + i, cy + i, &white); }
    for i in 6..=21 { set(cx + 7, cy + i, &white); }
    for i in 1..=6  { set(cx + i, cy + 18, &white); }
}

// ---------------------------------------------------------------------------
// Frame chunking
// ---------------------------------------------------------------------------

/// Split JPEG bytes into chunks with header.
///
/// Each chunk: [frame_id: u32 BE][chunk_idx: u16 BE][total: u16 BE][payload]
fn split_into_chunks(frame_id: u32, data: &[u8]) -> Vec<Vec<u8>> {
    let total = ((data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE) as u16;
    let mut chunks = Vec::with_capacity(total as usize);

    for (i, chunk_data) in data.chunks(CHUNK_SIZE).enumerate() {
        let mut chunk = Vec::with_capacity(8 + chunk_data.len());
        chunk.extend_from_slice(&frame_id.to_be_bytes());
        chunk.extend_from_slice(&(i as u16).to_be_bytes());
        chunk.extend_from_slice(&total.to_be_bytes());
        chunk.extend_from_slice(chunk_data);
        chunks.push(chunk);
    }

    chunks
}

// ---------------------------------------------------------------------------
// Data channel handlers
// ---------------------------------------------------------------------------

/// Handle incoming messages on the "input" data channel.
async fn handle_input_message(msg: DataChannelMessage, input: &InputEngine) {
    if msg.is_string {
        if let Ok(text) = String::from_utf8(msg.data.to_vec()) {
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
                        debug!("unhandled input message: {text}");
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
