//! Linux input injection via X11 (enigo crate).
//!
//! Uses `enigo` which wraps X11's XTest extension for keyboard/mouse injection.

use std::cell::RefCell;

use anyhow::Result;

use super::InputImpl;

use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

pub struct LinuxInput {
    enigo: RefCell<Enigo>,
}

impl LinuxInput {
    pub async fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("enigo init failed: {e}"))?;
        tracing::info!("Linux input ready (enigo/XTest)");
        Ok(Self { enigo: RefCell::new(enigo) })
    }
}

impl InputImpl for LinuxInput {
    fn send_key_event(&self, key_code: u16, pressed: bool) -> Result<()> {
        if let Some(key) = map_dom_keycode(key_code) {
            let dir = if pressed { Direction::Press } else { Direction::Release };
            self.enigo.borrow_mut().key(key, dir)?;
        }
        Ok(())
    }

    fn send_mouse_move(&self, x: f64, y: f64) -> Result<()> {
        self.enigo.borrow_mut().move_mouse(x as i32, y as i32, Coordinate::Abs)?;
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
        self.enigo.borrow_mut().button(btn, dir)?;
        Ok(())
    }

    fn send_mouse_scroll(&self, _dx: f64, dy: f64) -> Result<()> {
        let length = if dy > 0.0 { 1 } else { -1 };
        let steps = (dy.abs() / 50.0).ceil() as usize;
        for _ in 0..steps.max(1) {
            self.enigo.borrow_mut().scroll(length, Axis::Vertical)?;
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
        65 => Some(Key::Char('a')), 66 => Some(Key::Char('b')), 67 => Some(Key::Char('c')),
        68 => Some(Key::Char('d')), 69 => Some(Key::Char('e')), 70 => Some(Key::Char('f')),
        71 => Some(Key::Char('g')), 72 => Some(Key::Char('h')), 73 => Some(Key::Char('i')),
        74 => Some(Key::Char('j')), 75 => Some(Key::Char('k')), 76 => Some(Key::Char('l')),
        77 => Some(Key::Char('m')), 78 => Some(Key::Char('n')), 79 => Some(Key::Char('o')),
        80 => Some(Key::Char('p')), 81 => Some(Key::Char('q')), 82 => Some(Key::Char('r')),
        83 => Some(Key::Char('s')), 84 => Some(Key::Char('t')), 85 => Some(Key::Char('u')),
        86 => Some(Key::Char('v')), 87 => Some(Key::Char('w')), 88 => Some(Key::Char('x')),
        89 => Some(Key::Char('y')), 90 => Some(Key::Char('z')),

        // Numbers → Key::Char
        48 => Some(Key::Char('0')), 49 => Some(Key::Char('1')), 50 => Some(Key::Char('2')),
        51 => Some(Key::Char('3')), 52 => Some(Key::Char('4')), 53 => Some(Key::Char('5')),
        54 => Some(Key::Char('6')), 55 => Some(Key::Char('7')), 56 => Some(Key::Char('8')),
        57 => Some(Key::Char('9')),

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
        186 => Some(Key::Char(';')),  187 => Some(Key::Char('=')),
        188 => Some(Key::Char(',')),  189 => Some(Key::Char('-')),
        190 => Some(Key::Char('.')),  191 => Some(Key::Char('/')),
        192 => Some(Key::Char('`')),  219 => Some(Key::Char('[')),
        220 => Some(Key::Char('\\')), 221 => Some(Key::Char(']')),
        222 => Some(Key::Char('\'')),

        _ => {
            tracing::debug!("unmapped keycode: {code}");
            None
        }
    }
}
