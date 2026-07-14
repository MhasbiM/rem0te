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
    /// Cached X11 connection.
    #[cfg(feature = "x11-capture")]
    x11_conn: Option<Arc<Mutex<RustConnection>>>,
    /// Screen number.
    #[cfg(feature = "x11-capture")]
    x11_screen_num: usize,
    /// SHM segment (lazy-initialized on first capture).
    #[cfg(feature = "x11-capture")]
    shm: Mutex<Option<ShmState>>,
}

#[cfg(feature = "x11-capture")]
struct ShmState {
    seg: u32,       // X11 SHM segment ID (for x11rb)
    id: i32,        // OS shmget ID (for cleanup)
    ptr: *mut u8,   // mapped memory
    size: usize,    // segment size in bytes
}

// Safety: ShmState owns the shared memory mapping
#[cfg(feature = "x11-capture")]
unsafe impl Send for ShmState {}
#[cfg(feature = "x11-capture")]
unsafe impl Sync for ShmState {}

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
                #[cfg(feature = "x11-capture")]
                shm: Mutex::new(None),
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
            #[cfg(feature = "x11-capture")]
            shm: Mutex::new(None),
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
            return Some(CursorPosition {
                x: reply.root_x as u32,
                y: reply.root_y as u32,
            });
        }
        #[allow(unreachable_code)]
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
    use x11rb::protocol::shm::ConnectionExt as _;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

    let conn = cap.x11_conn.as_ref()
        .context("X11 connection not initialized")?;
    let conn = conn.lock().unwrap();

    let screen = &conn.setup().roots[cap.x11_screen_num];
    let root = screen.root;

    let geo = conn.get_geometry(root)?.reply()?;
    let w = geo.width as u32;
    let h = geo.height as u32;

    // Size: 32bpp for ZPixmap
    let size = (w as usize) * (h as usize) * 4;

    // Lazy-init SHM, fallback to regular GetImage
    let mut shm_guard = cap.shm.lock().unwrap();
    if shm_guard.is_none() {
        match create_shm(&conn, size) {
            Ok(s) => { tracing::info!("X11 SHM ready: {} bytes", size); *shm_guard = Some(s); }
            Err(e) => { tracing::warn!("SHM failed ({e}), using regular GetImage"); }
        }
    }

    let t0 = std::time::Instant::now();
    let raw = if let Some(ref shm) = *shm_guard {
        conn.shm_get_image(root, 0, 0, geo.width, geo.height, u32::MAX, ImageFormat::Z_PIXMAP.into(), shm.seg, 0)?.reply()?;
        unsafe { std::slice::from_raw_parts(shm.ptr, size) }
    } else {
        &conn.get_image(ImageFormat::Z_PIXMAP, root, 0, 0, geo.width, geo.height, u32::MAX)?.reply()?.data
    };
    let elapsed = t0.elapsed();
    tracing::debug!("capture: {}x{} in {:?} ({})", w, h, elapsed, if let Some(_) = *shm_guard { "SHM" } else { "GetImage" });
    let pixel_count = (w as usize) * (h as usize);

    // SHM ZPixmap: BGRX 32bpp → BGRA
    let mut bgra = Vec::with_capacity(pixel_count * 4);
    for chunk in raw.chunks_exact(4) {
        bgra.push(chunk[0]); // B
        bgra.push(chunk[1]); // G
        bgra.push(chunk[2]); // R
        bgra.push(255);      // A
    }
    for _ in (raw.len() / 4)..pixel_count { bgra.extend_from_slice(&[0,0,0,255]); }

    tracing::debug!("X11 SHM: {}x{} {}B → {}B in {:?}", w, h, raw.len(), bgra.len(), elapsed);
    Ok(CapturedFrame { data: bgra, width: w, height: h, timestamp: std::time::Instant::now() })
}

/// Create a shared memory segment and attach to X server.
#[cfg(feature = "x11-capture")]
fn create_shm(conn: &RustConnection, size: usize) -> Result<ShmState> {
    use x11rb::connection::Connection;
    use x11rb::protocol::shm::ConnectionExt as _;

    let id = unsafe { libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600) };
    if id < 0 {
        anyhow::bail!("shmget failed: {}", std::io::Error::last_os_error());
    }
    let ptr = unsafe { libc::shmat(id, std::ptr::null(), 0) };
    if ptr == libc::MAP_FAILED {
        unsafe { libc::shmctl(id, libc::IPC_RMID, std::ptr::null_mut()) };
        anyhow::bail!("shmat failed: {}", std::io::Error::last_os_error());
    }
    let seg = conn.generate_id()?;
    conn.shm_attach(seg, id as u32, false)?;
    Ok(ShmState { seg, id, ptr: ptr as *mut u8, size })
}

#[cfg(feature = "x11-capture")]
impl Drop for ShmState {
    fn drop(&mut self) {
        unsafe {
            libc::shmdt(self.ptr as *const _);
            libc::shmctl(self.id, libc::IPC_RMID, std::ptr::null_mut());
        }
    }
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
