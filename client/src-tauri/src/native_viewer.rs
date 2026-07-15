use std::sync::mpsc;

/// Start native GPU window for remote desktop display.
/// Works on macOS AND Linux — minifb handles threading per platform internally.
pub fn start_native_viewer(width: usize, height: usize) -> mpsc::Sender<Vec<u8>> {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    std::thread::spawn(move || {
        let mut window: Option<minifb::Window> = None;
        let mut pixels: Vec<u32> = Vec::new();
        let mut ww = width;
        let mut wh = height;

        for jpeg_data in rx {
            let img = match image::load_from_memory(&jpeg_data) {
                Ok(i) => i.to_rgba8(),
                Err(_) => continue,
            };
            let (iw, ih) = (img.width() as usize, img.height() as usize);

            if window.is_none() {
                ww = iw;
                wh = ih;
                pixels.resize(ww * wh, 0);
                window = minifb::Window::new(
                    "rem0te",
                    ww,
                    wh,
                    minifb::WindowOptions {
                        resize: true,
                        scale: minifb::Scale::FitScreen,
                        ..Default::default()
                    },
                ).ok();
            }

            if iw != ww || ih != wh {
                ww = iw;
                wh = ih;
                pixels.resize(ww * wh, 0);
            }

            let raw = img.into_raw();
            for (i, c) in raw.chunks_exact(4).enumerate() {
                if i < pixels.len() {
                    pixels[i] = (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32;
                }
            }

            if let Some(ref mut w) = window {
                if !w.is_open() { break; }
                w.update_with_buffer(&pixels, ww, wh).ok();
            }
        }
    });

    tx
}
