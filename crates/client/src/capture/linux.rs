//! Linux screen capture via PipeWire + xdg-desktop-portal.
//!
//! ## How it works (Wayland)
//! 1. Request a screen-cast session via `org.freedesktop.portal.ScreenCast`.
//! 2. The portal returns a PipeWire fd.
//! 3. Connect to PipeWire, negotiate video format, and read frames.
//!
//! ## Dependencies
//! - PipeWire running on the system
//! - xdg-desktop-portal (and a backend like `xdg-desktop-portal-gtk` or `-kde`)
//!
//! ## Implementation status
//! This is currently a **stub**. Real implementation requires:
//! - GStreamer pipeline: `pipewiresrc` → `videoconvert` → `appsink`
//! - Or direct PipeWire API via `pipewire` crate with DMA-BUF

use anyhow::Result;

use super::{CapturedFrame, CaptureImpl, CursorPosition};

pub struct LinuxCapture {
    display_width: u32,
    display_height: u32,
}

impl LinuxCapture {
    pub async fn new() -> Result<Self> {
        tracing::info!("initializing Linux PipeWire capture");

        // Detect display dimensions
        let (w, h) = detect_display_dimensions();

        // TODO: Full PipeWire implementation
        // 1. Connect to PipeWire daemon
        // 2. Request ScreenCast via xdg-desktop-portal (ashpd crate)
        // 3. When portal grants permission, receive PipeWire fd
        // 4. Create GStreamer pipeline or use pipewire crate directly
        // 5. Start reading frames

        Ok(Self {
            display_width: w,
            display_height: h,
        })
    }
}

impl CaptureImpl for LinuxCapture {
    fn display_dimensions(&self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }

    fn cursor_position(&self) -> Option<CursorPosition> {
        // TODO: Use libei or X11/xdotool to get cursor position
        None
    }

    fn capture_frame(&self) -> Result<CapturedFrame> {
        // TODO: Read frame from PipeWire/GStreamer pipeline
        // Placeholder: return a blank frame
        let size = (self.display_width * self.display_height * 4) as usize;
        Ok(CapturedFrame {
            data: vec![0u8; size],
            width: self.display_width,
            height: self.display_height,
            timestamp: std::time::Instant::now(),
        })
    }
}

/// Try to detect the current display dimensions on Linux.
fn detect_display_dimensions() -> (u32, u32) {
    // Try via xrandr (works on X11 and some Wayland compositors)
    if let Ok(output) = std::process::Command::new("xrandr")
        .arg("--current")
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(" connected") && line.contains('x') {
                // Parse: "HDMI-1 connected primary 1920x1080+0+0"
                if let Some(res) = line.split_whitespace().find(|w| w.contains('x') && !w.contains('+'))
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

    // Fallback via /sys/class/drm
    // TODO: iterate over /sys/class/drm/card*-*/modes

    // Hardcoded fallback
    (1920, 1080)
}
