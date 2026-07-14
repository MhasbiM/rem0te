//! Linux input via X11 XTest (x11rb). Stubs if x11-capture disabled.

use std::sync::Mutex;
use anyhow::Result;

#[cfg(feature = "x11-capture")]
use {x11rb::connection::Connection, x11rb::protocol::xtest::ConnectionExt as _,
     x11rb::protocol::xproto::ConnectionExt, x11rb::rust_connection::RustConnection};

use super::InputImpl;

pub struct LinuxInput {
    #[cfg(feature = "x11-capture")]
    conn: Mutex<RustConnection>,
    #[cfg(feature = "x11-capture")]
    root: u32,
}

impl LinuxInput {
    pub async fn new() -> Result<Self> {
        #[cfg(feature = "x11-capture")]
        {
            let d = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
            let (conn, sn) = RustConnection::connect(Some(&d))
                .map_err(|e| anyhow::anyhow!("X11 connect fail: {e}"))?;
            let root = conn.setup().roots[sn].root;
            tracing::info!("Linux input ready (XTest on {d})");
            return Ok(Self { conn: Mutex::new(conn), root });
        }
        #[cfg(not(feature = "x11-capture"))]
        {
            tracing::warn!("x11-capture disabled — input stubs");
            Ok(Self {})
        }
    }
}

impl InputImpl for LinuxInput {
    fn send_key_event(&self, key_code: u16, pressed: bool) -> Result<()> {
        #[cfg(feature = "x11-capture")]
        if let Some(xc) = dom_to_x11_keycode(key_code) {
            let c = self.conn.lock().unwrap();
            let t = if pressed { x11rb::protocol::xproto::KEY_PRESS_EVENT }
                    else { x11rb::protocol::xproto::KEY_RELEASE_EVENT };
            c.xtest_fake_input(t, xc as u32, 0, 0, 0, 0)?;
            c.flush()?;
        }
        Ok(())
    }

    fn send_mouse_move(&self, x: f64, y: f64) -> Result<()> {
        #[cfg(feature = "x11-capture")]
        {
            let c = self.conn.lock().unwrap();
            c.warp_pointer(x11rb::NONE, self.root, 0, 0, 0, 0, x as i16, y as i16)?;
            c.flush()?;
        }
        Ok(())
    }

    fn send_mouse_button(&self, button: u8, pressed: bool) -> Result<()> {
        #[cfg(feature = "x11-capture")]
        {
            let c = self.conn.lock().unwrap();
            let b = match button { 0 => 1, 1 => 3, 2 => 2, _ => return Ok(()) };
            let t = if pressed { x11rb::protocol::xproto::BUTTON_PRESS_EVENT }
                    else { x11rb::protocol::xproto::BUTTON_RELEASE_EVENT };
            c.xtest_fake_input(t, b, 0, 0, 0, 0)?;
            c.flush()?;
        }
        Ok(())
    }

    fn send_mouse_scroll(&self, _dx: f64, dy: f64) -> Result<()> {
        #[cfg(feature = "x11-capture")]
        {
            let c = self.conn.lock().unwrap();
            let b = if dy > 0.0 { 5u8 } else { 4u8 };
            for _ in 0..((dy.abs() / 50.0).ceil() as usize).max(1) {
                c.xtest_fake_input(x11rb::protocol::xproto::BUTTON_PRESS_EVENT, b as u32, 0, 0, 0, 0)?;
                c.xtest_fake_input(x11rb::protocol::xproto::BUTTON_RELEASE_EVENT, b as u32, 0, 0, 0, 0)?;
            }
            c.flush()?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DOM keycode → X11 keycode (US QWERTY, evdev keycodes)
// ---------------------------------------------------------------------------

#[cfg(feature = "x11-capture")]
fn dom_to_x11_keycode(code: u16) -> Option<u8> {
    match code {
        65 => Some(38), 66 => Some(56), 67 => Some(54), 68 => Some(40),
        69 => Some(26), 70 => Some(41), 71 => Some(42), 72 => Some(43),
        73 => Some(31), 74 => Some(44), 75 => Some(45), 76 => Some(46),
        77 => Some(58), 78 => Some(57), 79 => Some(32), 80 => Some(33),
        81 => Some(24), 82 => Some(27), 83 => Some(39), 84 => Some(28),
        85 => Some(30), 86 => Some(55), 87 => Some(25), 88 => Some(53),
        89 => Some(29), 90 => Some(52),

        49 => Some(10), 50 => Some(11), 51 => Some(12), 52 => Some(13),
        53 => Some(14), 54 => Some(15), 55 => Some(16), 56 => Some(17),
        57 => Some(18), 48 => Some(19),

        112 => Some(67), 113 => Some(68), 114 => Some(69),
        115 => Some(70), 116 => Some(71), 117 => Some(72),
        118 => Some(73), 119 => Some(74), 120 => Some(75),
        121 => Some(76), 122 => Some(95), 123 => Some(96),

        13 => Some(36), 27 => Some(9), 9 => Some(23), 32 => Some(65),
        8 => Some(22), 46 => Some(119),
        37 => Some(113), 38 => Some(111), 39 => Some(114), 40 => Some(116),
        16 => Some(50), 17 => Some(37), 18 => Some(64), 91 => Some(133),
        20 => Some(66), 36 => Some(110), 35 => Some(115),
        33 => Some(112), 34 => Some(117), 45 => Some(118),

        186 => Some(47), 187 => Some(21), 188 => Some(59), 189 => Some(20),
        190 => Some(60), 191 => Some(61), 192 => Some(49),
        219 => Some(34), 220 => Some(51), 221 => Some(35), 222 => Some(48),

        _ => {
            tracing::debug!("unmapped keycode: {code}");
            None
        }
    }
}
