use anyhow::Result;
use std::io::Read;

mod capture;
mod relay;
mod signaling;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: rem0te-target <server_addr> <peer_id>");
        return Ok(());
    }
    let server_addr = &args[1];
    let peer_id = &args[2];
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());

    log::info!("Target waiting for viewer on {}...", server_addr);

    let (sig, relay_addr, session_id) = signaling::Signaling::connect(server_addr, peer_id).await?;
    let mut relay = relay::RelayClient::connect_target(&relay_addr, &session_id).await?;

    log::info!("Starting ffmpeg on {}...", display);
    let mut child = capture::start_ffmpeg(&display)?;
    let stdout = child.stdout.take().unwrap();

    // Read ffmpeg stdout in blocking task
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut buf: Vec<u8> = Vec::with_capacity(512 * 1024);
        let mut chunk = vec![0u8; 65536];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.len() > 4_000_000 { buf.clear(); }
                    while let Some(frame) = capture::extract_frame(&mut buf) {
                        let _ = tx.blocking_send(frame);
                    }
                }
                Err(_) => break,
            }
        }
    });

    log::info!("Streaming full HD via ffmpeg MJPEG...");
    while let Some(frame) = rx.recv().await {
        relay.send_frame(&frame).await?;
    }

    drop(sig);
    Ok(())
}
