use anyhow::Result;

mod capture;
mod relay;
mod signaling;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: rem0te-target <server_addr> <peer_id>");
        eprintln!("  server_addr: signaling server (e.g. 192.168.1.1:21118)");
        eprintln!("  peer_id:     our peer ID (e.g. peer-linux-01)");
        return Ok(());
    }

    let server_addr = &args[1];
    let peer_id = &args[2];

    log::info!("rem0te-target: waiting for viewer connection...");

    let (relay_addr, session_id) = signaling::wait_for_relay_info(server_addr, peer_id).await?;

    // Connect to relay
    let mut relay = relay::RelayClient::connect_target(&relay_addr, &session_id).await?;

    // Start capture + stream
    let mut cap = capture::ScreenCapture::new(1920, 1080);
    cap.start()?;

    log::info!("Streaming started! (50% scale, mozjpeg q40)");
    loop {
        let frame = cap.capture_frame()?;
        relay.send_frame(&frame).await?;
        tokio::task::yield_now().await;
    }
}
