//! Linux screen capture.
//!
//! ## Strategies (auto-detected)
//! 1. **X11** (default on Ubuntu ≤22): `x11rb` pure-Rust capture — zero system deps
//! 2. **Wayland** (future): PipeWire + xdg-desktop-portal via GStreamer (`linux-media` feature)
//! 3. **Fallback**: blank frames
//!
//! Auto-detection: checks `$XDG_SESSION_TYPE` and `$WAYLAND_DISPLAY`.

use anyhow::{Context, Result};

use super::{CapturedFrame, CaptureImpl, CursorPosition};

pub struct LinuxCapture {
    display_width: u32,
    display_height: u32,
    /// Whether X11 capture is available (X11 session + x11-capture feature).
    is_x11: bool,
}

impl LinuxCapture {
    pub async fn new() -> Result<Self> {
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
        let is_wayland = session == "wayland" || !wayland_display.is_empty();

        let (w, h) = detect_display_dimensions();

        if is_wayland {
            tracing::info!("Wayland detected — screen capture not yet implemented. Enable `linux-media` feature for PipeWire support.");
        } else {
            tracing::info!("X11 detected — using x11rb for screen capture");
        }

        Ok(Self {
            display_width: w,
            display_height: h,
            is_x11: !is_wayland && cfg!(feature = "x11-capture"),
        })
    }
}

impl CaptureImpl for LinuxCapture {
    fn display_dimensions(&self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }

    fn cursor_position(&self) -> Option<CursorPosition> {
        // TODO: query X11 pointer position
        None
    }

    fn capture_frame(&self) -> Result<CapturedFrame> {
        if self.is_x11 {
            capture_x11()
        } else {
            // Stub: blank frame
            let size = (self.display_width * self.display_height * 4) as usize;
            Ok(CapturedFrame {
                data: vec![0u8; size],
                width: self.display_width,
                height: self.display_height,
                timestamp: std::time::Instant::now(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// X11 capture (behind x11-capture feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "x11-capture")]
fn capture_x11() -> Result<CapturedFrame> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
    use x11rb::rust_connection::RustConnection;

    let (conn, screen_num) = RustConnection::connect(None)
        .context("failed to connect to X11 server — is DISPLAY set?")?;

    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    let geo = conn.get_geometry(root)?.reply()?;
    let w = geo.width as u32;
    let h = geo.height as u32;

    let reply = conn
        .get_image(
            ImageFormat::Z_PIXMAP,
            root,
            0,
            0,
            geo.width,
            geo.height,
            u32::MAX,
        )?
        .reply()?;

    // X11 ZPixmap: BGRX (little-endian) → convert to BGRA
    let raw = reply.data;
    let pixel_count = (w as usize) * (h as usize);
    let mut bgra = Vec::with_capacity(pixel_count * 4);

    for chunk in raw.chunks_exact(4) {
        bgra.push(chunk[0]); // B
        bgra.push(chunk[1]); // G
        bgra.push(chunk[2]); // R
        bgra.push(255);      // A (opaque)
    }

    // Pad if image depth is 24-bit (3 bytes/pixel)
    let full_chunks = raw.len() / 4;
    if full_chunks < pixel_count {
        let remaining = pixel_count - full_chunks;
        for _ in 0..remaining {
            bgra.extend_from_slice(&[0, 0, 0, 255]);
        }
    }

    Ok(CapturedFrame {
        data: bgra,
        width: w,
        height: h,
        timestamp: std::time::Instant::now(),
    })
}

#[cfg(not(feature = "x11-capture"))]
fn capture_x11() -> Result<CapturedFrame> {
    anyhow::bail!("x11-capture feature not enabled");
}

// ---------------------------------------------------------------------------
// Display detection
// ---------------------------------------------------------------------------

fn detect_display_dimensions() -> (u32, u32) {
    if let Ok(output) = std::process::Command::new("xrandr")
        .arg("--current")
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(" connected") && line.contains('x') {
                if let Some(res) = line
                    .split_whitespace()
                    .find(|w| w.contains('x') && !w.contains('+'))
                {
                    let parts: Vec<&str> = res.split('x').collect();
                    if parts.len() == 2 {
                        if let (Ok(w), Ok(h)) = (parts[0].parse(), parts[1].parse()) {
                            return (w, h);
                        }
                    }
                }
            }
        }
    }
    (1920, 1080)
}
