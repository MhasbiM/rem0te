use anyhow::Result;

/// Screen capture module supporting macOS (screencapture CLI) and Linux (X11 via x11rb)
pub struct ScreenCapture {
    active: bool,
    display_id: u32,
    width: u32,
    height: u32,
    last_frame: Vec<u8>,
}

impl ScreenCapture {
    pub fn new() -> Self {
        Self { active: false, display_id: 0, width: 1920, height: 1080, last_frame: Vec::new() }
    }

    pub fn start(&mut self) -> Result<()> {
        log::info!("Starting screen capture...");
        self.active = true;

        #[cfg(target_os = "macos")]
        {
            let did = unsafe { core_graphics::display::CGMainDisplayID() };
            self.display_id = did;
            self.width = unsafe { core_graphics::display::CGDisplayPixelsWide(did) } as u32;
            self.height = unsafe { core_graphics::display::CGDisplayPixelsHigh(did) } as u32;
            log::info!("macOS display: {}x{}", self.width, self.height);
        }

        #[cfg(target_os = "linux")]
        {
            self.width = 1920;
            self.height = 1080;
            log::info!("Linux capture initialized");
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        log::info!("Stopping screen capture");
        self.active = false;
        Ok(())
    }

    pub fn capture_frame(&mut self) -> Result<Vec<u8>> {
        if !self.active {
            return Err(anyhow::anyhow!("Screen capture not active"));
        }
        self.do_capture()
    }

    fn do_capture(&mut self) -> Result<Vec<u8>> {
        #[cfg(target_os = "macos")]
        { return self.capture_macos(); }

        #[cfg(target_os = "linux")]
        { return self.capture_linux(); }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        { self.generate_placeholder() }
    }

    // ─── macOS: use screencapture CLI tool ──────────────────────────

    #[cfg(target_os = "macos")]
    fn capture_macos(&mut self) -> Result<Vec<u8>> {
        use std::process::Command;
        let tmp = std::env::temp_dir().join(format!("rem0te_{}.jpg", std::process::id()));
        let status = Command::new("screencapture")
            .args(["-x", "-t", "jpg", "-T", "0", tmp.to_str().unwrap()])
            .status()
            .map_err(|e| anyhow::anyhow!("screencapture: {}", e))?;
        if !status.success() {
            return Err(anyhow::anyhow!("screencapture failed"));
        }
        let data = std::fs::read(&tmp).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp);
        if data.is_empty() { return self.generate_placeholder(); }
        self.last_frame = data.clone();
        Ok(data)
    }

    // ─── Linux ──────────────────────────────────────────────────────

    #[cfg(target_os = "linux")]
    fn capture_linux(&mut self) -> Result<Vec<u8>> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            return self.capture_wayland();
        }
        self.capture_x11()
    }

    #[cfg(target_os = "linux")]
    fn capture_x11(&mut self) -> Result<Vec<u8>> {
        use image::ImageEncoder;
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::*;
        use x11rb::rust_connection::RustConnection;

        let (conn, screen_num) = RustConnection::connect(None)
            .map_err(|e| anyhow::anyhow!("X11 connect: {}", e))?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        let geo = conn.get_geometry(root)?.reply()?;
        self.width = geo.width as u32;
        self.height = geo.height as u32;

        let img_data = conn
            .get_image(ImageFormat::Z_PIXMAP, root, 0, 0, geo.width, geo.height, u32::MAX)?
            .reply()?
            .data;

        let mut jpeg = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 70);
        encoder.write_image(&img_data, geo.width as u32, geo.height as u32, image::ExtendedColorType::Rgba8)?;
        self.last_frame = jpeg.clone();
        Ok(jpeg)
    }

    #[cfg(target_os = "linux")]
    fn capture_wayland(&mut self) -> Result<Vec<u8>> {
        log::warn!("Wayland capture: PipeWire not yet implemented");
        self.generate_placeholder()
    }

    // ─── Placeholder ────────────────────────────────────────────────

    fn generate_placeholder(&mut self) -> Result<Vec<u8>> {
        use image::ImageEncoder;
        let w = self.width.max(320);
        let h = self.height.max(240);
        let mut img = image::ImageBuffer::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let r = (x as f32 / w as f32 * 30.0) as u8;
            let g = (y as f32 / h as f32 * 50.0) as u8;
            let b = 40 + (x as f32 / w as f32 * 20.0) as u8;
            *p = image::Rgba([r, g, b, 255]);
        }
        let mut png = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png);
        encoder.write_image(&img, w, h, image::ExtendedColorType::Rgba8)?;
        Ok(png)
    }
}
