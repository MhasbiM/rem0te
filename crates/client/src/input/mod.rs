//! Remote input injection.
//!
//! Provides a unified interface for simulating keyboard and mouse events
//! on the remote machine:
//! - Linux: libei (Wayland-native) or uinput (kernel-level)
//! - macOS: CGEvent (CoreGraphics)

use anyhow::Result;

// Platform-specific implementations
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::LinuxInput;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::MacOsInput;

/// Engine for injecting input events on the local machine.
pub struct InputEngine {
    #[cfg(target_os = "linux")]
    inner: LinuxInput,

    #[cfg(target_os = "macos")]
    inner: MacOsInput,
}

impl InputEngine {
    /// Create a new input engine for the current platform.
    pub async fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                inner: LinuxInput::new().await?,
            })
        }

        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                inner: MacOsInput::new().await?,
            })
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("unsupported platform: {}", std::env::consts::OS);
        }
    }

    /// Send a key press or release event.
    pub async fn send_key_event(&self, key_code: u16, pressed: bool) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.inner.send_key_event(key_code, pressed)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        anyhow::bail!("unsupported");
    }

    /// Move the mouse cursor to absolute coordinates.
    pub async fn send_mouse_move(&self, x: f64, y: f64) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.inner.send_mouse_move(x, y)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        anyhow::bail!("unsupported");
    }

    /// Send a mouse button press or release.
    pub async fn send_mouse_button(&self, button: u8, pressed: bool) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.inner.send_mouse_button(button, pressed)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        anyhow::bail!("unsupported");
    }

    /// Send a mouse scroll event.
    pub async fn send_mouse_scroll(&self, dx: f64, dy: f64) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.inner.send_mouse_scroll(dx, dy)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        anyhow::bail!("unsupported");
    }
}

// ---------------------------------------------------------------------------
// Internal trait
// ---------------------------------------------------------------------------

#[allow(dead_code)]
trait InputImpl {
    fn send_key_event(&self, key_code: u16, pressed: bool) -> Result<()>;
    fn send_mouse_move(&self, x: f64, y: f64) -> Result<()>;
    fn send_mouse_button(&self, button: u8, pressed: bool) -> Result<()>;
    fn send_mouse_scroll(&self, dx: f64, dy: f64) -> Result<()>;
}
