use std::sync::mpsc;

#[cfg(target_os = "macos")]
#[link(name = "System", kind = "dylib")]
extern "C" {
    fn dispatch_async_f(queue: *mut std::ffi::c_void, ctx: *mut std::ffi::c_void, work: extern "C" fn(*mut std::ffi::c_void));
    fn dispatch_get_main_queue() -> *mut std::ffi::c_void;
}

pub fn start_native_viewer(width: usize, height: usize) -> mpsc::Sender<Vec<u8>> {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    #[cfg(target_os = "macos")]
    {
        extern "C" fn render(ctx: *mut std::ffi::c_void) {
            let rx = unsafe { Box::from_raw(ctx as *mut mpsc::Receiver<Vec<u8>>) };
            render_loop(*rx, 960, 540);
        }
        let bx = Box::new(rx);
        unsafe { dispatch_async_f(dispatch_get_main_queue(), Box::into_raw(bx) as *mut std::ffi::c_void, render); }
    }

    #[cfg(not(target_os = "macos"))]
    {
        std::thread::spawn(move || render_loop(rx, width, height));
    }

    tx
}

fn render_loop(rx: mpsc::Receiver<Vec<u8>>, mut ww: usize, mut wh: usize) {
    let mut window: Option<minifb::Window> = None;
    let mut pixels: Vec<u32> = Vec::new();
    for jpeg_data in rx {
        let img = match image::load_from_memory(&jpeg_data) {
            Ok(i) => i.to_rgba8(), Err(_) => continue,
        };
        let (iw, ih) = (img.width() as usize, img.height() as usize);
        if window.is_none() {
            ww = iw; wh = ih; pixels.resize(ww * wh, 0);
            window = minifb::Window::new("rem0te", ww, wh,
                minifb::WindowOptions { resize: true, scale: minifb::Scale::FitScreen, ..Default::default() }).ok();
        }
        if iw != ww || ih != wh { ww = iw; wh = ih; pixels.resize(ww * wh, 0); }
        let raw = img.into_raw();
        for (i, c) in raw.chunks_exact(4).enumerate() {
            if i < pixels.len() { pixels[i] = (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32; }
        }
        if let Some(ref mut w) = window {
            if !w.is_open() { break; }
            w.update_with_buffer(&pixels, ww, wh).ok();
        }
    }
}
