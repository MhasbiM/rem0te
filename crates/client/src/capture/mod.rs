//! Screen capture abstraction.
//!
//! Provides a unified interface for capturing the desktop across platforms:
//! - Linux: PipeWire + xdg-desktop-portal
//! - macOS: CoreGraphics (CGDisplay)

use anyhow::Result;

// Platform-specific implementations
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::LinuxCapture;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::MacOsCapture;

/// A single captured frame.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    /// Raw pixel data in BGRA32 format.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Timestamp of capture.
    pub timestamp: std::time::Instant,
}

/// Engine for capturing the desktop screen.
///
/// Platform-specific implementation is selected at compile time.
pub struct CaptureEngine {
    #[cfg(target_os = "linux")]
    inner: LinuxCapture,

    #[cfg(target_os = "macos")]
    inner: MacOsCapture,
}

impl CaptureEngine {
    /// Create a new capture engine for the current platform.
    pub async fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                inner: LinuxCapture::new().await?,
            })
        }

        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                inner: MacOsCapture::new().await?,
            })
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("unsupported platform: {}", std::env::consts::OS);
        }
    }

    /// Get the current display dimensions.
    pub fn display_dimensions(&self) -> (u32, u32) {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.inner.display_dimensions()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            (1920, 1080)
        }
    }

    /// Capture a single frame from the display.
    pub fn capture_frame(&self) -> Result<CapturedFrame> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.inner.capture_frame()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("unsupported platform");
        }
    }

    /// Start continuous frame capture, calling `on_frame` for each frame.
    /// Runs until the returned cancellation token is dropped.
    pub async fn start_streaming<F>(&self, fps: u32, on_frame: F) -> Result<()>
    where
        F: Fn(CapturedFrame) + Send + 'static,
    {
        let interval = std::time::Duration::from_secs_f64(1.0 / fps as f64);
        let mut tick = tokio::time::interval(interval);

        loop {
            tick.tick().await;
            match self.capture_frame() {
                Ok(frame) => {
                    on_frame(frame);
                }
                Err(e) => {
                    tracing::warn!("frame capture error: {e}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Trait for platform implementations (used internally)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
trait CaptureImpl {
    fn display_dimensions(&self) -> (u32, u32);
    fn capture_frame(&self) -> Result<CapturedFrame>;
}
