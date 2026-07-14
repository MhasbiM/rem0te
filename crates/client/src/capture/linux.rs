//! Linux screen capture.
//!
//! ## Strategies (auto-detected)
//! 1. **X11**: `x11rb` pure-Rust capture — tries $DISPLAY, :0, :1
//! 2. **Wayland** (future): PipeWire + GStreamer (`linux-media` feature)
//! 3. **Fallback**: blank frames (allows client to run, just no video)

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};

use super::{CapturedFrame, CaptureImpl, CursorPosition};

pub struct LinuxCapture {
    display_width: u32,
    display_height: u32,
    /// Whether X11 capture was successfully initialized.
    x11_available: AtomicBool,
    /// Cached X11 display name for reconnection each frame.
    x11_display: Option<String>,
}

impl LinuxCapture {
    pub async fn new() -> Result<Self> {
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
        let is_wayland = session == "wayland" || !wayland_display.is_empty();

        let (w, h) = detect_display_dimensions();

        if is_wayland {
            tracing::info!("Wayland detected — screen capture not yet implemented. Enable `linux-media` feature for PipeWire support.");
            return Ok(Self {
                display_width: w,
                display_height: h,
                x11_available: AtomicBool::new(false),
                x11_display: None,
            });
        }

        // Try to find a working X11 display
        let display = find_x11_display();
        if let Some(ref dpy) = display {
            tracing::info!("X11 display '{}' — using x11rb for screen capture", dpy);
        } else {
            tracing::warn!("No X11 display found (try: export DISPLAY=:0). Falling back to blank frames.");
        }

        Ok(Self {
            display_width: w,
            display_height: h,
            x11_available: AtomicBool::new(display.is_some() && cfg!(feature = "x11-capture")),
            x11_display: display,
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
        if self.x11_available.load(Ordering::Relaxed) {
            match capture_x11(self.x11_display.as_deref()) {
                Ok(frame) => return Ok(frame),
                Err(e) => {
                    tracing::warn!("X11 capture failed, falling back to blank: {e}");
                    self.x11_available.store(false, Ordering::Relaxed);
                }
            }
        }

        // Blank frame fallback
        blank_frame(self.display_width, self.display_height)
    }
}

// ---------------------------------------------------------------------------
// X11 capture (behind x11-capture feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "x11-capture")]
fn capture_x11(display: Option<&str>) -> Result<CapturedFrame> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
    use x11rb::rust_connection::RustConnection;

    let (conn, screen_num) = RustConnection::connect(display)
        .context("failed to connect to X11 server — try: export DISPLAY=:0")?;

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
        for _ in full_chunks..pixel_count {
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
fn capture_x11(_display: Option<&str>) -> Result<CapturedFrame> {
    anyhow::bail!("x11-capture feature not enabled");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find a working X11 display. Tries $DISPLAY, then probes common defaults.
fn find_x11_display() -> Option<String> {
    if let Ok(dpy) = std::env::var("DISPLAY") {
        if !dpy.is_empty() {
            return Some(dpy);
        }
    }

    #[cfg(feature = "x11-capture")]
    for candidate in &[":0", ":0.0", ":1", ":1.0"] {
        if x11rb::rust_connection::RustConnection::connect(Some(candidate)).is_ok() {
            return Some(candidate.to_string());
        }
    }

    None
}

/// Return a blank (black) frame.
fn blank_frame(w: u32, h: u32) -> Result<CapturedFrame> {
    Ok(CapturedFrame {
        data: vec![0u8; (w * h * 4) as usize],
        width: w,
        height: h,
        timestamp: std::time::Instant::now(),
    })
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
