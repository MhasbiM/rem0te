//! Linux input injection via X11 (enigo crate).
//!
//! Uses `enigo` which wraps X11's XTest extension for keyboard/mouse injection.

use anyhow::Result;

use super::InputImpl;

use enigo::{Axis, Coordinate, Direction, Enigo, Key, Mouse, Settings};

pub struct LinuxInput {
    enigo: Enigo,
}

impl LinuxInput {
    pub async fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("enigo init failed: {e}"))?;
        tracing::info!("Linux input ready (enigo/XTest)");
        Ok(Self { enigo })
    }
}

impl InputImpl for LinuxInput {
    fn send_key_event(&self, key_code: u16, pressed: bool) -> Result<()> {
        if let Some(key) = map_dom_keycode(key_code) {
            let dir = if pressed { Direction::Press } else { Direction::Release };
            self.enigo.key(key, dir)?;
        }
        Ok(())
    }

    fn send_mouse_move(&self, x: f64, y: f64) -> Result<()> {
        self.enigo.move_mouse(x as i32, y as i32, Coordinate::Abs)?;
        Ok(())
    }

    fn send_mouse_button(&self, button: u8, pressed: bool) -> Result<()> {
        let btn = match button {
            0 => enigo::Button::Left,
            1 => enigo::Button::Right,
            2 => enigo::Button::Middle,
            _ => return Ok(()),
        };
        let dir = if pressed { Direction::Press } else { Direction::Release };
        self.enigo.button(btn, dir)?;
        Ok(())
    }

    fn send_mouse_scroll(&self, _dx: f64, dy: f64) -> Result<()> {
        let length = if dy > 0.0 { 1 } else { -1 };
        let steps = (dy.abs() / 50.0).ceil() as usize;
        for _ in 0..steps.max(1) {
            self.enigo.scroll(length, Axis::Vertical)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DOM keycode → enigo::Key (subset — covers common keys)
// ---------------------------------------------------------------------------

fn map_dom_keycode(code: u16) -> Option<Key> {
    match code {
        65 => Some(Key::A), 66 => Some(Key::B), 67 => Some(Key::C),
        68 => Some(Key::D), 69 => Some(Key::E), 70 => Some(Key::F),
        71 => Some(Key::G), 72 => Some(Key::H), 73 => Some(Key::I),
        74 => Some(Key::J), 75 => Some(Key::K), 76 => Some(Key::L),
        77 => Some(Key::M), 78 => Some(Key::N), 79 => Some(Key::O),
        80 => Some(Key::P), 81 => Some(Key::Q), 82 => Some(Key::R),
        83 => Some(Key::S), 84 => Some(Key::T), 85 => Some(Key::U),
        86 => Some(Key::V), 87 => Some(Key::W), 88 => Some(Key::X),
        89 => Some(Key::Y), 90 => Some(Key::Z),

        48 => Some(Key::Num0), 49 => Some(Key::Num1), 50 => Some(Key::Num2),
        51 => Some(Key::Num3), 52 => Some(Key::Num4), 53 => Some(Key::Num5),
        54 => Some(Key::Num6), 55 => Some(Key::Num7), 56 => Some(Key::Num8),
        57 => Some(Key::Num9),

        112 => Some(Key::F1), 113 => Some(Key::F2), 114 => Some(Key::F3),
        115 => Some(Key::F4), 116 => Some(Key::F5), 117 => Some(Key::F6),
        118 => Some(Key::F7), 119 => Some(Key::F8), 120 => Some(Key::F9),
        121 => Some(Key::F10), 122 => Some(Key::F11), 123 => Some(Key::F12),

        13 => Some(Key::Return),  27 => Some(Key::Escape),
        9  => Some(Key::Tab),     32 => Some(Key::Space),
        8  => Some(Key::Backspace), 46 => Some(Key::Delete),
        37 => Some(Key::LeftArrow),  38 => Some(Key::UpArrow),
        39 => Some(Key::RightArrow), 40 => Some(Key::DownArrow),
        16 => Some(Key::Shift),   17 => Some(Key::Control),
        18 => Some(Key::Alt),     91 => Some(Key::Meta),
        20 => Some(Key::CapsLock),
        36 => Some(Key::Home),    35 => Some(Key::End),
        33 => Some(Key::PageUp),  34 => Some(Key::PageDown),
        45 => Some(Key::Insert),

        _ => {
            tracing::debug!("unmapped keycode: {code}");
            None
        }
    }
}
