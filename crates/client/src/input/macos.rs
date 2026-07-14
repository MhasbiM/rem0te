//! macOS input injection via CoreGraphics `CGEvent`.
//!
//! ## How it works
//! 1. Create a `CGEvent` with the desired type (key down/up, mouse move, etc.).
//! 2. Set event properties (keycode, mouse position, button number).
//! 3. Post the event to the system event stream using `CGEventPost`.
//!
//! ## Permissions
//! The app needs **Accessibility** permission (under System Settings →
//! Privacy & Security → Accessibility). Without it, `CGEventPost` will
//! silently fail.
//!
//! ## Keycode mapping
//! Web `key_code` values use the standard USB HID / DOM `KeyboardEvent.code`
//! convention. These need to be mapped to macOS keycodes (which use a
//! different numbering scheme). A mapping table is maintained below.

use anyhow::{anyhow, Result};
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use super::InputImpl;

pub struct MacOsInput;

impl MacOsInput {
    pub async fn new() -> Result<Self> {
        tracing::info!("initializing macOS CGEvent input engine");
        // Verify accessibility permissions
        match CGEventSource::new(CGEventSourceStateID::Private) {
            Ok(_) => tracing::info!("accessibility permission OK"),
            Err(_) => {
                tracing::warn!(
                    "⚠️  Accessibility permission may not be granted. \
                     Go to System Settings → Privacy → Accessibility and add this app."
                );
            }
        }

        Ok(Self)
    }

    /// Create an event source, trying HIDSystemState first, falling back to Private.
    fn create_event_source() -> Result<CGEventSource> {
        CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .or_else(|_| CGEventSource::new(CGEventSourceStateID::Private))
            .map_err(|_| anyhow!("failed to create CGEventSource — check Accessibility permissions"))
    }
}

impl InputImpl for MacOsInput {
    fn send_key_event(&self, key_code: u16, pressed: bool) -> Result<()> {
        let mac_keycode = map_keycode_to_macos(key_code);
        let event_source = Self::create_event_source()?;

        let event = CGEvent::new_keyboard_event(event_source, mac_keycode, pressed)
            .map_err(|_| anyhow!("failed to create keyboard event"))?;

        event.post(CGEventTapLocation::HID);
        tracing::debug!("key: code={key_code} → mac={mac_keycode}, pressed={pressed}");
        Ok(())
    }

    fn send_mouse_move(&self, x: f64, y: f64) -> Result<()> {
        let event_source = Self::create_event_source()?;

        let event = CGEvent::new_mouse_event(
            event_source,
            CGEventType::MouseMoved,
            core_graphics::geometry::CGPoint::new(x, y),
            CGMouseButton::Left,
        )
        .map_err(|_| anyhow!("failed to create mouse move event"))?;

        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn send_mouse_button(&self, button: u8, pressed: bool) -> Result<()> {
        let cg_button = match button {
            0 => CGMouseButton::Left,
            1 => CGMouseButton::Right,
            2 => CGMouseButton::Center,
            _ => {
                tracing::warn!("unknown mouse button: {button}");
                return Ok(());
            }
        };

        // Use a default position — mouse_move should be called first to set position
        let current_pos = core_graphics::geometry::CGPoint::new(0.0, 0.0);

        let event_source = Self::create_event_source()?;
        let event_type = if pressed {
            match cg_button {
                CGMouseButton::Right => CGEventType::RightMouseDown,
                _ => CGEventType::LeftMouseDown,
            }
        } else {
            match cg_button {
                CGMouseButton::Right => CGEventType::RightMouseUp,
                _ => CGEventType::LeftMouseUp,
            }
        };

        let event = CGEvent::new_mouse_event(event_source, event_type, current_pos, cg_button)
            .map_err(|_| anyhow!("failed to create mouse button event"))?;

        event.post(CGEventTapLocation::HID);
        tracing::debug!("mouse button: btn={button}, pressed={pressed}");
        Ok(())
    }

    fn send_mouse_scroll(&self, dx: f64, dy: f64) -> Result<()> {
        let event_source = Self::create_event_source()?;

        let event = CGEvent::new_scroll_event(
            event_source,
            ScrollEventUnit::PIXEL,
            2, // wheel count
            dy as i32,
            dx as i32,
            0,
        )
        .map_err(|_| anyhow!("failed to create scroll event"))?;

        event.post(CGEventTapLocation::HID);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Keycode mapping (simplified — expand as needed)
// ---------------------------------------------------------------------------

/// Map a subset of USB HID / DOM key codes to macOS virtual keycodes.
///
/// Full table: <https://developer.apple.com/library/archive/documentation/
///              Cocoa/Conceptual/EventOverview/TextDefaultsBindings/TextDefaultsBindings.html>
fn map_keycode_to_macos(web_code: u16) -> u16 {
    match web_code {
        // Letters
        65 => 0x00,  // A
        66 => 0x0B,  // B
        67 => 0x08,  // C
        68 => 0x02,  // D
        69 => 0x0E,  // E
        70 => 0x03,  // F
        71 => 0x05,  // G
        72 => 0x04,  // H
        73 => 0x22,  // I
        74 => 0x26,  // J
        75 => 0x28,  // K
        76 => 0x25,  // L
        77 => 0x2E,  // M
        78 => 0x2D,  // N
        79 => 0x1F,  // O
        80 => 0x23,  // P
        81 => 0x0C,  // Q
        82 => 0x0F,  // R
        83 => 0x01,  // S
        84 => 0x11,  // T
        85 => 0x20,  // U
        86 => 0x09,  // V
        87 => 0x0D,  // W
        88 => 0x07,  // X
        89 => 0x10,  // Y
        90 => 0x06,  // Z

        // Numbers
        48 => 0x1D,  // 0
        49 => 0x12,  // 1
        50 => 0x13,  // 2
        51 => 0x14,  // 3
        52 => 0x15,  // 4
        53 => 0x17,  // 5
        54 => 0x16,  // 6
        55 => 0x1A,  // 7
        56 => 0x1C,  // 8
        57 => 0x19,  // 9

        // Special keys
        13 => 0x24,  // Enter / Return
        27 => 0x35,  // Escape
        9  => 0x30,  // Tab
        32 => 0x31,  // Space
        8  => 0x33,  // Backspace
        46 => 0x75,  // Delete
        37 => 0x7B,  // Left arrow
        38 => 0x7E,  // Up arrow
        39 => 0x7C,  // Right arrow
        40 => 0x7D,  // Down arrow
        16 => 0x38,  // Shift (left)
        17 => 0x3B,  // Control (left)
        18 => 0x3A,  // Option / Alt (left)
        91 => 0x37,  // Command / Super (left)

        _ => {
            tracing::debug!("unmapped keycode: {web_code}");
            0
        }
    }
}
