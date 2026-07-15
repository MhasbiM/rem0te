use anyhow::{Context, Result};

#[cfg(target_os = "linux")]
pub struct ScreenCapture {
    x11: Option<x11rb::rust_connection::RustConnection>,
    screen_num: usize,
    width: u32,
    height: u32,
}

#[cfg(not(target_os = "linux"))]
pub struct ScreenCapture { width: u32, height: u32 }

impl ScreenCapture {
    pub fn new(width: u32, height: u32) -> Self {
        #[cfg(target_os = "linux")]
        { Self { x11: None, screen_num: 0, width, height } }
        #[cfg(not(target_os = "linux"))]
        { Self { width, height } }
    }

    pub fn start(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            use x11rb::connection::Connection;
            let (conn, screen_num) = x11rb::rust_connection::RustConnection::connect(None)?;
            let screen = &conn.setup().roots[screen_num];
            self.width = screen.width_in_pixels as u32;
            self.height = screen.height_in_pixels as u32;
            self.x11 = Some(conn);
            self.screen_num = screen_num;
            log::info!("Capture: {}x{}", self.width, self.height);
        }
        Ok(())
    }

    pub fn capture_frame(&mut self) -> Result<Vec<u8>> {
        #[cfg(target_os = "linux")]
        {
            use x11rb::connection::Connection;
            use x11rb::protocol::xproto::*;
            let conn = self.x11.as_ref().context("X11 not connected")?;
            let screen = &conn.setup().roots[self.screen_num];
            let root = screen.root;
            let geo = conn.get_geometry(root)?.reply()?;
            let raw = conn.get_image(ImageFormat::Z_PIXMAP, root, 0, 0, geo.width, geo.height, u32::MAX)?.reply()?.data;

            // 50% downscale + mozjpeg quality 60 (balance speed/quality)
            let (sw, sh) = (geo.width as u32 / 2, geo.height as u32 / 2);
            let rgb = bgra_to_rgb_scaled(&raw, geo.width as u32, geo.height as u32, sw, sh);

            let mut jpeg = Vec::new();
            let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
            comp.set_size(sw as usize, sh as usize);
            comp.set_quality(60.0);
            comp.set_fastest_defaults();
            let mut comp = comp.start_compress(&mut jpeg).context("JPEG compress")?;
            comp.write_scanlines(&rgb).context("JPEG write")?;
            comp.finish().context("JPEG finish")?;
            Ok(jpeg)
        }
        #[cfg(not(target_os = "linux"))]
        { Ok(vec![]) }
    }
}

fn bgra_to_rgb_full(bgra: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    let row = (w * 4) as usize;
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = y * row + x * 4;
            if i + 3 < bgra.len() { rgb.push(bgra[i+2]); rgb.push(bgra[i+1]); rgb.push(bgra[i]); }
        }
    }
    rgb
}

fn bgra_to_rgb_scaled(bgra: &[u8], w: u32, h: u32, sw: u32, sh: u32) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((sw * sh * 3) as usize);
    let row_bytes = (w * 4) as usize;
    for sy in 0..sh as usize {
        let y = sy * h as usize / sh as usize;
        let row = y * row_bytes;
        for sx in 0..sw as usize {
            let x = sx * w as usize / sw as usize;
            let i = row + x * 4;
            if i + 3 < bgra.len() { rgb.push(bgra[i+2]); rgb.push(bgra[i+1]); rgb.push(bgra[i]); }
        }
    }
    rgb
}
