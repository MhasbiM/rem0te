//! Linux screen capture.
//!
//! ## Strategies (auto-detected)
//! 1. **X11**: `x11rb` pure-Rust capture — tries $DISPLAY, :0, :1
//! 2. **Wayland** (future): PipeWire + GStreamer (`linux-media` feature)
//! 3. **Fallback**: blank frames (allows client to run, just no video)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};

use super::{CapturedFrame, CaptureImpl, CursorPosition};

#[cfg(feature = "x11-capture")]
use x11rb::rust_connection::RustConnection;

pub struct LinuxCapture {
    display_width: u32,
    display_height: u32,
    /// Whether X11 capture is available.
    x11_available: AtomicBool,
    /// Cached X11 connection (opened once, reused across frames).
    #[cfg(feature = "x11-capture")]
    x11_conn: Option<Arc<Mutex<RustConnection>>>,
    /// Screen number for the cached connection.
    #[cfg(feature = "x11-capture")]
    x11_screen_num: usize,
}

impl LinuxCapture {
    pub async fn new() -> Result<Self> {
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
        let is_wayland = session == "wayland" || !wayland_display.is_empty();

        let (w, h) = detect_display_dimensions();

        if is_wayland {
            tracing::info!("Wayland detected — screen capture not yet implemented.");
            return Ok(Self {
                display_width: w,
                display_height: h,
                x11_available: AtomicBool::new(false),
                #[cfg(feature = "x11-capture")]
                x11_conn: None,
                #[cfg(feature = "x11-capture")]
                x11_screen_num: 0,
            });
        }

        // Try to connect to X11 once and cache the connection
        #[cfg(feature = "x11-capture")]
        let (conn, screen_num, ok) = {
            let mut ok = false;
            let mut conn = None;
            let mut sn = 0;

            // Try $DISPLAY first, then common fallbacks
            let displays: Vec<String> = {
                let mut v = Vec::new();
                if let Ok(d) = std::env::var("DISPLAY") { if !d.is_empty() { v.push(d); } }
                v.push(":0".into());
                v.push(":1".into());
                v.push(":0.0".into());
                v
            };

            for dpy in &displays {
                tracing::info!("trying X11 display '{}'...", dpy);
                match RustConnection::connect(Some(dpy.as_str())) {
                    Ok((c, s)) => {
                        tracing::info!("X11 connected to '{}' — capture ENABLED", dpy);
                        conn = Some(Arc::new(Mutex::new(c)));
                        sn = s;
                        ok = true;
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("X11 '{}' failed: {e}", dpy);
                    }
                }
            }

            if !ok {
                tracing::error!(
                    "Cannot connect to any X11 display. Tried: {:?}. \
                     Check: echo $DISPLAY, or try: export DISPLAY=:0",
                    displays
                );
            }

            (conn, sn, ok)
        };

        #[cfg(not(feature = "x11-capture"))]
        let ok = false;

        Ok(Self {
            display_width: w,
            display_height: h,
            x11_available: AtomicBool::new(ok),
            #[cfg(feature = "x11-capture")]
            x11_conn: conn,
            #[cfg(feature = "x11-capture")]
            x11_screen_num: screen_num,
        })
    }
}

impl CaptureImpl for LinuxCapture {
    fn display_dimensions(&self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }

    fn cursor_position(&self) -> Option<CursorPosition> {
        #[cfg(feature = "x11-capture")]
        {
            use x11rb::connection::Connection;
            use x11rb::protocol::xproto::ConnectionExt;
            let conn = self.x11_conn.as_ref()?;
            let conn = conn.lock().ok()?;
            let screen = &conn.setup().roots[self.x11_screen_num];
            let reply = conn.query_pointer(screen.root).ok()?.reply().ok()?;
            tracing::trace!("cursor at ({}, {})", reply.root_x, reply.root_y);
            return Some(CursorPosition {
                x: reply.root_x as u32,
                y: reply.root_y as u32,
            });
        }
        None
    }

    fn capture_frame(&self) -> Result<CapturedFrame> {
        if self.x11_available.load(Ordering::Relaxed) {
            match capture_x11_cached(self) {
                Ok(frame) => return Ok(frame),
                Err(e) => {
                    tracing::warn!("X11 capture failed, falling back to blank: {e}");
                    self.x11_available.store(false, Ordering::Relaxed);
                }
            }
        }
        blank_frame(self.display_width, self.display_height)
    }
}

// ---------------------------------------------------------------------------
// X11 capture (behind x11-capture feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "x11-capture")]
fn capture_x11_cached(cap: &LinuxCapture) -> Result<CapturedFrame> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

    let conn = cap.x11_conn.as_ref()
        .context("X11 connection not initialized")?;
    let conn = conn.lock().unwrap();

    let screen = &conn.setup().roots[cap.x11_screen_num];
    let root = screen.root;

    let geo = conn.get_geometry(root)?.reply()?;
    let w = geo.width as u32;
    let h = geo.height as u32;
    let depth = geo.depth;

    let t0 = std::time::Instant::now();
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

    let raw = reply.data;
    let pixel_count = (w as usize) * (h as usize);
    let bpp = raw.len() / pixel_count; // actual bytes per pixel

    let bgra = match bpp {
        4 => {
            // BGRX (32-bit, X = unused/padding)
            let mut out = Vec::with_capacity(pixel_count * 4);
            for chunk in raw.chunks_exact(4) {
                out.push(chunk[0]); // B
                out.push(chunk[1]); // G
                out.push(chunk[2]); // R
                out.push(255);      // A
            }
            let full = raw.len() / 4;
            for _ in full..pixel_count {
                out.extend_from_slice(&[0, 0, 0, 255]);
            }
            out
        }
        3 => {
            // BGR (24-bit, tightly packed)
            let mut out = Vec::with_capacity(pixel_count * 4);
            for chunk in raw.chunks_exact(3) {
                out.push(chunk[0]); out.push(chunk[1]); out.push(chunk[2]);
                out.push(255);
            }
            out
        }
        2 => {
            // RGB565 (16-bit)
            let mut out = Vec::with_capacity(pixel_count * 4);
            for chunk in raw.chunks_exact(2) {
                let p = u16::from_le_bytes([chunk[0], chunk[1]]);
                let r = ((p >> 11) & 0x1F) as u8 * 255 / 31;
                let g = ((p >> 5) & 0x3F) as u8 * 255 / 63;
                let b = (p & 0x1F) as u8 * 255 / 31;
                out.push(b); out.push(g); out.push(r); out.push(255);
            }
            out
        }
        n => anyhow::bail!(
            "unexpected X11 bytes-per-pixel: {n} (depth={depth}, raw={}B, w={w}, h={h})",
            raw.len()
        ),
    };

    let elapsed = t0.elapsed();
    tracing::debug!(
        "X11 capture: {}x{} depth={} bpp={} {}B → {}B BGRA in {:?}",
        w, h, depth, bpp, raw.len(), bgra.len(), elapsed
    );

    Ok(CapturedFrame {
        data: bgra,
        width: w,
        height: h,
        timestamp: std::time::Instant::now(),
    })
}

#[cfg(not(feature = "x11-capture"))]
fn capture_x11_cached(_cap: &LinuxCapture) -> Result<CapturedFrame> {
    anyhow::bail!("x11-capture feature not enabled");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
