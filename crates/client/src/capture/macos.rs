//! macOS screen capture via CoreGraphics.
//!
//! ## How it works
//! 1. Enumerate online displays using `CGGetActiveDisplayList`.
//! 2. For each display, create a `CGImage` via `CGDisplayCreateImage`.
//! 3. Extract raw pixel data (BGRA32).
//! 4. Compose all displays into a single frame (or just use the main display).
//!
//! ## Future improvements
//! - Use **ScreenCaptureKit** (macOS 12.3+) for better performance and
//!   per-window capture. Requires bridging to Swift/ObjC via `objc2` crate.
//! - Use hardware-accelerated encoding via VideoToolbox.
//!
//! ## Permissions
//! The app needs **Screen Recording** permission. On first run, macOS will
//! prompt the user. The binary must be signed with the appropriate entitlements:
//! ```xml
//! <key>com.apple.security.device.camera</key><true/>
//! ```

use anyhow::{Context, Result};
use core_graphics::display::CGDisplay;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::image::CGImage;

use super::{CapturedFrame, CaptureImpl, CursorPosition};

pub struct MacOsCapture {
    display_width: u32,
    display_height: u32,
}

impl MacOsCapture {
    pub async fn new() -> Result<Self> {
        tracing::info!("initializing macOS CoreGraphics capture");

        let (w, h) = detect_display_dimensions();

        tracing::info!("detected display: {w}x{h}");

        Ok(Self {
            display_width: w,
            display_height: h,
        })
    }
}

impl CaptureImpl for MacOsCapture {
    fn display_dimensions(&self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }

    fn cursor_position(&self) -> Option<CursorPosition> {
        // Create a NULL event to query current cursor location
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
        let event = CGEvent::new(source).ok()?;
        let pos = event.location();
        Some(CursorPosition {
            x: pos.x as u32,
            y: pos.y as u32,
        })
    }

    fn capture_frame(&self) -> Result<CapturedFrame> {
        // Get the main display
        let display = CGDisplay::main();

        // Capture the display as a CGImage
        let image: CGImage = display
            .image()
            .context("failed to capture main display")?;

        let width = image.width() as u32;
        let height = image.height() as u32;
        let bpr = image.bytes_per_row();

        // Extract pixel data via CFData
        let cf_data = image.data();
        let bytes = cf_data.bytes();

        if bytes.is_empty() {
            anyhow::bail!("CGImage data is empty");
        }

        // Copy row by row (bytes_per_row may include padding)
        let pixel_count = (width as usize) * (height as usize) * 4;
        let mut pixel_data = Vec::with_capacity(pixel_count);

        for row in 0..height as usize {
            let start = row * bpr;
            let end = start + (width as usize) * 4;
            if end <= bytes.len() {
                pixel_data.extend_from_slice(&bytes[start..end]);
            }
        }

        tracing::debug!(
            width,
            height,
            "captured frame: {} bytes",
            pixel_data.len()
        );

        Ok(CapturedFrame {
            data: pixel_data,
            width,
            height,
            timestamp: std::time::Instant::now(),
        })
    }
}

/// Detect the main display dimensions on macOS.
fn detect_display_dimensions() -> (u32, u32) {
    let display = CGDisplay::main();
    let bounds = display.bounds();
    (bounds.size.width as u32, bounds.size.height as u32)
}
