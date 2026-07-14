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

    // ─── macOS: native CoreGraphics (fast, in-memory) ──────────────

    #[cfg(target_os = "macos")]
    fn capture_macos(&mut self) -> Result<Vec<u8>> {
        // Direct FFI to CoreGraphics — fastest path
        extern "C" {
            fn CGMainDisplayID() -> u32;
            fn CGDisplayCreateImage(display: u32) -> *mut std::ffi::c_void;
            fn CGImageGetWidth(image: *mut std::ffi::c_void) -> usize;
            fn CGImageGetHeight(image: *mut std::ffi::c_void) -> usize;
            fn CGImageGetBytesPerRow(image: *mut std::ffi::c_void) -> usize;
            fn CGImageGetDataProvider(image: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn CGDataProviderCopyData(provider: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn CGImageRelease(image: *mut std::ffi::c_void);
            fn CFDataGetLength(data: *mut std::ffi::c_void) -> isize;
            fn CFDataGetBytePtr(data: *mut std::ffi::c_void) -> *const u8;
            fn CFRelease(cf: *mut std::ffi::c_void);
        }

        let display_id = unsafe { CGMainDisplayID() };
        let image = unsafe { CGDisplayCreateImage(display_id) };
        if image.is_null() {
            return self.capture_macos_fallback();
        }

        let width = unsafe { CGImageGetWidth(image) } as u32;
        let height = unsafe { CGImageGetHeight(image) } as u32;
        let bpr = unsafe { CGImageGetBytesPerRow(image) };

        let provider = unsafe { CGImageGetDataProvider(image) };
        if provider.is_null() {
            unsafe { CGImageRelease(image); }
            return self.capture_macos_fallback();
        }

        let cf_data = unsafe { CGDataProviderCopyData(provider) };
        if cf_data.is_null() {
            unsafe { CGImageRelease(image); }
            return self.capture_macos_fallback();
        }

        let data_len = unsafe { CFDataGetLength(cf_data) } as usize;
        let data_ptr = unsafe { CFDataGetBytePtr(cf_data) };

        // BGRA → RGB
        let pixels = (width * height) as usize;
        let mut rgb = Vec::with_capacity(pixels * 3);
        unsafe {
            for y in 0..height as usize {
                let row_start = y * bpr;
                for x in 0..width as usize {
                    let p = row_start + x * 4;
                    if p + 3 < data_len {
                        rgb.push(*data_ptr.add(p + 2)); // R
                        rgb.push(*data_ptr.add(p + 1)); // G
                        rgb.push(*data_ptr.add(p));     // B
                    }
                }
            }
        }

        unsafe {
            CFRelease(cf_data);
            CGImageRelease(image);
        }

        self.width = width;
        self.height = height;

        let mut jpeg = Vec::new();
        {
            use image::ImageEncoder;
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 50);
            encoder.write_image(&rgb, width, height, image::ExtendedColorType::Rgb8)?;
        }
        self.last_frame = jpeg.clone();
        Ok(jpeg)
    }

    /// CLI fallback (slow but works on any macOS version)
    #[cfg(target_os = "macos")]
    fn capture_macos_fallback(&mut self) -> Result<Vec<u8>> {
        use std::process::Command;
        let tmp = std::env::temp_dir().join(format!("rem0te_{}.jpg", std::process::id()));
        let _ = Command::new("screencapture")
            .args(["-x", "-t", "jpg", "-T", "0", tmp.to_str().unwrap()])
            .status();
        let data = std::fs::read(&tmp).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp);
        if data.is_empty() { return self.generate_placeholder(); }
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

        let raw = conn
            .get_image(ImageFormat::Z_PIXMAP, root, 0, 0, geo.width, geo.height, u32::MAX)?
            .reply()?
            .data;

        // X11 Z_PIXMAP returns BGRA (4 bytes per pixel), JPEG needs RGB
        let rgb = bgra_to_rgb(&raw, geo.width as u32, geo.height as u32);

        let mut jpeg = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 70);
        encoder.write_image(&rgb, geo.width as u32, geo.height as u32, image::ExtendedColorType::Rgb8)?;
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

/// Convert BGRA raw bytes (X11 Z_PIXMAP format) to RGB (JPEG-compatible)
fn bgra_to_rgb(bgra: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixels = (width * height) as usize;
    let mut rgb = Vec::with_capacity(pixels * 3);
    // BGRA: bytes per row may include padding, but for now assume tightly packed
    let row_bytes = (width * 4) as usize;
    for y in 0..height as usize {
        let row_start = y * row_bytes;
        for x in 0..width as usize {
            let i = row_start + x * 4;
            if i + 3 < bgra.len() {
                rgb.push(bgra[i + 2]); // R (from B)
                rgb.push(bgra[i + 1]); // G
                rgb.push(bgra[i]);     // B (from R)
            } else {
                rgb.push(0);
                rgb.push(0);
                rgb.push(0);
            }
        }
    }
    rgb
}
