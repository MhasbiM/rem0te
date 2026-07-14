//! Video encoding pipeline: BGRA → I420 (YUV) → AV1 bitstream.
//!
//! Uses `rav1e` (pure Rust AV1 encoder) at speed preset 10 (fastest).
//! Modern browsers decode AV1 natively via WebRTC (Chrome 90+, Firefox 113+).

use anyhow::{Context as _, Result};
use tracing::{debug, warn};

// rav1e types — use explicit paths since the prelude has naming conflicts
use rav1e::{Config, Context, EncoderConfig, EncoderStatus};
use rav1e::data::{FrameParameters, FrameType, Rational};
use rav1e::color::ChromaSampling;
use rav1e::prelude::{FrameTypeOverride, Tune};

// ---------------------------------------------------------------------------
// BGRA → I420 color conversion
// ---------------------------------------------------------------------------

/// Convert a BGRA (8-bit per channel) frame to planar I420 (YUV 4:2:0).
///
/// Returns (y_plane, u_plane, v_plane) as separate Vec<u8>.
/// I420 layout: full-resolution Y plane, then subsampled U and V planes
/// (each 1/4 the size of Y).
pub fn bgra_to_i420_planes(
    bgra: &[u8],
    width: u32,
    height: u32,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_w = w / 2;
    let uv_h = h / 2;
    let uv_size = uv_w * uv_h;

    let mut y_plane = vec![0u8; y_size];
    let mut u_plane = vec![0u8; uv_size];
    let mut v_plane = vec![0u8; uv_size];

    for row in 0..h {
        for col in 0..w {
            let px = (row * w + col) * 4;
            let b = bgra[px] as f32;
            let g = bgra[px + 1] as f32;
            let r = bgra[px + 2] as f32;

            // BT.601 full-range Y
            let y = 0.299f32.mul_add(r, 0.587f32.mul_add(g, 0.114 * b));
            y_plane[row * w + col] = y.clamp(0.0, 255.0) as u8;

            // Subsample U and V every 2×2 pixels
            if row % 2 == 0 && col % 2 == 0 {
                let uv_row = row / 2;
                let uv_col = col / 2;

                let u = (-0.169f32).mul_add(r, (-0.331f32).mul_add(g, 0.5f32.mul_add(b, 128.0)));
                let v = 0.5f32.mul_add(r, (-0.419f32).mul_add(g, (-0.081f32).mul_add(b, 128.0)));

                u_plane[uv_row * uv_w + uv_col] = u.clamp(0.0, 255.0) as u8;
                v_plane[uv_row * uv_w + uv_col] = v.clamp(0.0, 255.0) as u8;
            }
        }
    }

    (y_plane, u_plane, v_plane)
}

// ---------------------------------------------------------------------------
// AV1 Encoder (rav1e)
// ---------------------------------------------------------------------------

/// Wrapper around rav1e AV1 encoder for real-time screen capture.
pub struct Av1Encoder {
    ctx: Context<u8>,
    width: usize,
    height: usize,
    /// Next keyframe forced after this many frames.
    frames_since_key: u64,
    keyframe_interval: u64,
}

impl Av1Encoder {
    /// Create a new AV1 encoder configured for real-time screen sharing.
    ///
    /// * `width`, `height` — frame dimensions in pixels.
    /// * `fps` — target framerate (for timebase calculation).
    /// * `bitrate_kbps` — target bitrate in kbps.
    pub fn new(width: u32, height: u32, fps: u32, bitrate_kbps: u32) -> Result<Self> {
        let width = width as usize;
        let height = height as usize;

        // Ensure even dimensions (required by chroma subsampling).
        let width = width.saturating_sub(width % 2);
        let height = height.saturating_sub(height % 2);

        let mut enc_config = EncoderConfig::with_speed_preset(10); // Fastest
        enc_config.width = width;
        enc_config.height = height;
        enc_config.time_base = Rational::new(1, fps as u64);
        enc_config.bit_depth = 8;
        enc_config.chroma_sampling = ChromaSampling::Cs420;
        enc_config.low_latency = true;
        enc_config.error_resilient = true;
        enc_config.min_key_frame_interval = 0;
        enc_config.max_key_frame_interval = (fps * 2) as u64;
        enc_config.sample_aspect_ratio = Rational::new(1, 1);
        enc_config.tune = Tune::Psnr;
        enc_config.still_picture = false;
        enc_config.bitrate = (bitrate_kbps * 1000) as i32;

        let cfg = Config::new().with_encoder_config(enc_config);
        let ctx: Context<u8> = cfg
            .new_context()
            .context("failed to create AV1 encoder context")?;

        Ok(Self {
            ctx,
            width,
            height,
            // Start at keyframe_interval so the FIRST frame is always a keyframe.
            // Browser decoders cannot start decoding without a keyframe first.
            frames_since_key: (fps * 2) as u64,
            keyframe_interval: (fps * 2) as u64,
        })
    }

    /// Encode one I420 frame to AV1 bitstream.
    ///
    /// Returns the encoded frame data, or `None` if encoder deferred output
    /// (normal for first few frames — encoder builds up lookahead).
    pub fn encode(
        &mut self,
        y: &[u8],
        u: &[u8],
        v: &[u8],
    ) -> Result<Option<EncodedFrame>> {
        let mut frame = self.ctx.new_frame();

        // Fill Y plane
        frame.planes[0].copy_from_raw_u8(y, self.width, 1);

        // Fill U and V planes (subsampled 2×2)
        let uv_width = self.width / 2;
        let uv_height = self.height / 2;
        frame.planes[1].copy_from_raw_u8(u, uv_width, 1);
        frame.planes[2].copy_from_raw_u8(v, uv_width, 1);

        // Pad invisible borders (required by encoder)
        frame.planes[0].pad(self.width, self.height);
        frame.planes[1].pad(uv_width, uv_height);
        frame.planes[2].pad(uv_width, uv_height);

        // Determine if this should be a keyframe
        let params = if self.frames_since_key >= self.keyframe_interval {
            self.frames_since_key = 0;
            FrameParameters {
                frame_type_override: FrameTypeOverride::Key,
                ..Default::default()
            }
        } else {
            FrameParameters::default()
        };
        self.frames_since_key += 1;

        self.ctx
            .send_frame((frame, params))
            .context("failed to send frame to AV1 encoder")?;

        // Receive all available encoded packets.
        // Must loop because receive_packet() can return:
        // - Ok(packet): frame ready → keep looping (multiple may be queued)
        // - Err(NeedMoreData): no more output for now → stop
        // - Err(Encoded): frame encoded internally but not emitted → keep trying
        loop {
            match self.ctx.receive_packet() {
                Ok(packet) => {
                    let is_key = packet.frame_type == FrameType::KEY;
                    debug!(
                        "AV1: {} bytes, key={}, type={:?}",
                        packet.data.len(),
                        is_key,
                        packet.frame_type,
                    );
                    return Ok(Some(EncodedFrame {
                        data: packet.data.to_vec(),
                        keyframe: is_key,
                    }));
                }
                Err(EncoderStatus::NeedMoreData) => {
                    // No packet ready — need more input frames
                    return Ok(None);
                }
                Err(EncoderStatus::Encoded) => {
                    // Frame was encoded but not emitted yet — keep trying
                    continue;
                }
                Err(EncoderStatus::LimitReached) => {
                    return Ok(None);
                }
                Err(e) => {
                    warn!("AV1 receive_packet error: {:?}", e);
                    return Ok(None);
                }
            }
        }
    }

    /// Flush remaining frames at end of stream.
    #[allow(dead_code)]
    pub fn flush(&mut self) -> Vec<Vec<u8>> {
        self.ctx.flush();
        let mut packets = Vec::new();
        loop {
            match self.ctx.receive_packet() {
                Ok(packet) => packets.push(packet.data.to_vec()),
                Err(EncoderStatus::NeedMoreData)
                | Err(EncoderStatus::Encoded) => continue,
                Err(_) => break,
            }
        }
        packets
    }
}

/// An encoded video frame ready to be sent via WebRTC.
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub keyframe: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgra_to_i420_sizes() {
        let w = 64u32;
        let h = 48u32;
        let bgra = vec![0u8; (w * h * 4) as usize];
        let (y, u, v) = bgra_to_i420_planes(&bgra, w, h);
        assert_eq!(y.len(), (w * h) as usize);
        assert_eq!(u.len(), ((w / 2) * (h / 2)) as usize);
        assert_eq!(v.len(), ((w / 2) * (h / 2)) as usize);
    }
}
