use anyhow::Result;
use std::sync::mpsc;

/// Native GPU-accelerated window for remote desktop display
pub fn spawn_native_viewer(width: usize, height: usize) -> Result<mpsc::Sender<Vec<u8>>> {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    std::thread::spawn(move || {
        render_loop(rx, width, height);
    });

    Ok(tx)
}

fn render_loop(rx: mpsc::Receiver<Vec<u8>>, mut win_w: usize, mut win_h: usize) {
    let mut window: Option<minifb::Window> = None;
    let mut pixels: Vec<u32> = Vec::new();

    for jpeg_data in rx {
        // Decode JPEG
        let img = match image::load_from_memory(&jpeg_data) {
            Ok(img) => img.to_rgba8(),
            Err(_) => continue,
        };

        let (iw, ih) = (img.width() as usize, img.height() as usize);

        // Create window on first frame
        if window.is_none() {
            win_w = iw;
            win_h = ih;
            pixels.resize(win_w * win_h, 0);
            window = minifb::Window::new(
                "rem0te - Remote Desktop",
                win_w,
                win_h,
                minifb::WindowOptions {
                    resize: true,
                    scale: minifb::Scale::FitScreen,
                    ..Default::default()
                },
            ).ok();
        }

        // Resize buffer if needed
        if iw != win_w || ih != win_h {
            win_w = iw;
            win_h = ih;
            pixels.resize(win_w * win_h, 0);
        }

        // RGBA → 0RGB (u32)
        let raw = img.into_raw();
        for (i, chunk) in raw.chunks_exact(4).enumerate() {
            if i < pixels.len() {
                pixels[i] = (chunk[0] as u32) << 16  // R
                          | (chunk[1] as u32) << 8   // G
                          | (chunk[2] as u32);        // B
            }
        }

        if let Some(ref mut w) = window {
            if !w.is_open() { break; }
            w.update_with_buffer(&pixels, win_w, win_h).ok();
        }
    }
}
