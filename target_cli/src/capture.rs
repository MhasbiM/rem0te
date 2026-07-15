use anyhow::{Context, Result};
use std::io::Read;

pub fn start_ffmpeg(display: &str) -> Result<std::process::Child> {
    std::process::Command::new("ffmpeg")
        .args([
            "-f", "x11grab", "-video_size", "1920x1080", "-framerate", "30",
            "-i", display, "-f", "mjpeg", "-q:v", "6", "-threads", "0", "-an", "pipe:1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("ffmpeg not found: sudo apt install ffmpeg")
}

pub fn extract_frame(buf: &mut Vec<u8>) -> Option<Vec<u8>> {
    let soi = buf.windows(2).position(|w| w == [0xFF, 0xD8])?;
    let eoi = buf[soi+2..].windows(2).position(|w| w == [0xFF, 0xD9])?;
    let end = soi + 2 + eoi + 2;
    let frame = buf[..end].to_vec();
    buf.drain(..end);
    if frame.len() > 500 { Some(frame) } else { None }
}
