//! Linux input injection.
//!
//! ## Strategy
//! - **Primary (Wayland)**: `libei` — the official Wayland protocol for
//!   emulated input. Supported by GNOME 45+ and KDE Plasma 6+.
//! - **Fallback**: `uinput` — kernel-level virtual input device.
//!   Requires write access to `/dev/uinput` (group `input` or root).
//!
//! ## Setup
//! ```bash
//! # Add user to input group
//! sudo usermod -a -G input $USER
//! # Or set udev rule
//! echo 'KERNEL=="uinput", MODE="0660", GROUP="input"' | sudo tee /etc/udev/rules.d/99-uinput.rules
//! ```
//!
//! ## Implementation status
//! This is a **stub**. Real implementation requires:
//! - `libei` crate integration for Wayland compositors
//! - `evdev` crate for uinput device creation
//! - Keycode mapping from web standard to Linux evdev codes

use anyhow::Result;

use super::InputImpl;

pub struct LinuxInput;

impl LinuxInput {
    pub async fn new() -> Result<Self> {
        tracing::info!("initializing Linux input engine");

        // TODO: Initialize libei connection or create uinput device
        // 1. Try to connect to libei socket ($XDG_RUNTIME_DIR/libei-*)
        // 2. If libei fails, try to open /dev/uinput
        // 3. Create virtual keyboard and mouse devices

        Ok(Self)
    }
}

impl InputImpl for LinuxInput {
    fn send_key_event(&self, key_code: u16, _pressed: bool) -> Result<()> {
        // TODO: Map web key_code to Linux evdev keycode and emit via uinput/libei
        tracing::debug!("key event: code={key_code}");
        Ok(())
    }

    fn send_mouse_move(&self, x: f64, y: f64) -> Result<()> {
        // TODO: Emit REL_X / REL_Y via uinput, or absolute position via libei
        tracing::debug!("mouse move: x={x}, y={y}");
        Ok(())
    }

    fn send_mouse_button(&self, button: u8, pressed: bool) -> Result<()> {
        // TODO: Emit BTN_LEFT / BTN_RIGHT / BTN_MIDDLE
        tracing::debug!("mouse button: btn={button}, pressed={pressed}");
        Ok(())
    }

    fn send_mouse_scroll(&self, dx: f64, dy: f64) -> Result<()> {
        // TODO: Emit REL_WHEEL / REL_HWHEEL
        tracing::debug!("mouse scroll: dx={dx}, dy={dy}");
        Ok(())
    }
}
