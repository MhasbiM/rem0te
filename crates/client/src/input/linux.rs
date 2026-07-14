//! Linux input injection via X11 (enigo crate).
//!
//! Uses `enigo` which wraps X11's XTest extension for keyboard/mouse injection.

use std::sync::Mutex;

use anyhow::Result;

use super::InputImpl;

use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

pub struct LinuxInput {
    enigo: Mutex<Enigo>,
}

impl LinuxInput {
    pub async fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("enigo init failed: {e}"))?;
        tracing::info!("Linux input ready (enigo/XTest)");
        Ok(Self { enigo: Mutex::new(enigo) })
    }
}

impl InputImpl for LinuxInput {
    fn send_key_event(&self, key_code: u16, pressed: bool) -> Result<()> {
        if let Some(key) = map_dom_keycode(key_code) {
            let dir = if pressed { Direction::Press } else { Direction::Release };
            self.enigo.lock().unwrap().key(key, dir)?;
        }
        Ok(())
    }

    fn send_mouse_move(&self, x: f64, y: f64) -> Result<()> {
        self.enigo.lock().unwrap().move_mouse(x as i32, y as i32, Coordinate::Abs)?;
        Ok(())
    }

    fn send_mouse_button(&self, button: u8, pressed: bool) -> Result<()> {
        let btn = match button {
            0 => Button::Left,
            1 => Button::Right,
            2 => Button::Middle,
            _ => return Ok(()),
        };
        let dir = if pressed { Direction::Press } else { Direction::Release };
        self.enigo.lock().unwrap().button(btn, dir)?;
        Ok(())
    }

    fn send_mouse_scroll(&self, _dx: f64, dy: f64) -> Result<()> {
        let length = if dy > 0.0 { 1 } else { -1 };
        let steps = (dy.abs() / 50.0).ceil() as usize;
        for _ in 0..steps.max(1) {
            self.enigo.lock().unwrap().scroll(length, Axis::Vertical)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DOM keycode → enigo::Key (subset — covers common keys)
// ---------------------------------------------------------------------------

fn map_dom_keycode(code: u16) -> Option<Key> {
    match code {
        // Letters → Key::Char
        65 => Some(Key::Layout('a')), 66 => Some(Key::Layout('b')), 67 => Some(Key::Layout('c')),
        68 => Some(Key::Layout('d')), 69 => Some(Key::Layout('e')), 70 => Some(Key::Layout('f')),
        71 => Some(Key::Layout('g')), 72 => Some(Key::Layout('h')), 73 => Some(Key::Layout('i')),
        74 => Some(Key::Layout('j')), 75 => Some(Key::Layout('k')), 76 => Some(Key::Layout('l')),
        77 => Some(Key::Layout('m')), 78 => Some(Key::Layout('n')), 79 => Some(Key::Layout('o')),
        80 => Some(Key::Layout('p')), 81 => Some(Key::Layout('q')), 82 => Some(Key::Layout('r')),
        83 => Some(Key::Layout('s')), 84 => Some(Key::Layout('t')), 85 => Some(Key::Layout('u')),
        86 => Some(Key::Layout('v')), 87 => Some(Key::Layout('w')), 88 => Some(Key::Layout('x')),
        89 => Some(Key::Layout('y')), 90 => Some(Key::Layout('z')),

        // Numbers → Key::Char
        48 => Some(Key::Layout('0')), 49 => Some(Key::Layout('1')), 50 => Some(Key::Layout('2')),
        51 => Some(Key::Layout('3')), 52 => Some(Key::Layout('4')), 53 => Some(Key::Layout('5')),
        54 => Some(Key::Layout('6')), 55 => Some(Key::Layout('7')), 56 => Some(Key::Layout('8')),
        57 => Some(Key::Layout('9')),

        // Function keys
        112 => Some(Key::F1), 113 => Some(Key::F2), 114 => Some(Key::F3),
        115 => Some(Key::F4), 116 => Some(Key::F5), 117 => Some(Key::F6),
        118 => Some(Key::F7), 119 => Some(Key::F8), 120 => Some(Key::F9),
        121 => Some(Key::F10), 122 => Some(Key::F11), 123 => Some(Key::F12),

        // Special keys
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

        // Symbols → Key::Char
        186 => Some(Key::Layout(';')),  187 => Some(Key::Layout('=')),
        188 => Some(Key::Layout(',')),  189 => Some(Key::Layout('-')),
        190 => Some(Key::Layout('.')),  191 => Some(Key::Layout('/')),
        192 => Some(Key::Layout('`')),  219 => Some(Key::Layout('[')),
        220 => Some(Key::Layout('\\')), 221 => Some(Key::Layout(']')),
        222 => Some(Key::Layout('\'')),

        _ => {
            tracing::debug!("unmapped keycode: {code}");
            None
        }
    }
}
