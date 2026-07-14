//! rem0te client — remote desktop agent.
//!
//! This binary runs on the machine to be controlled. It:
//! 1. Connects to the signaling server via WebSocket
//! 2. Registers this machine as available for remote control
//! 3. On incoming connection, sets up WebRTC and starts streaming
//! 4. Receives and executes input events
//!
//! ## Platform support
//! - **Linux (Wayland)**: PipeWire screen capture, libei/uinput for input
//! - **macOS**: CoreGraphics screen capture, CGEvent for input

mod config;
mod capture;
mod input;
mod signaling;
mod webrtc;
mod video;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Logging ───────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rem0te_client=debug,info")),
        )
        .init();

    // ── Config ────────────────────────────────────────────────────
    let config = Config::parse();

    tracing::info!("🖥️  rem0te remote agent starting");
    tracing::info!("   server: {}", config.server);
    tracing::info!("   os: {} ({})", std::env::consts::OS, std::env::consts::ARCH);

    // ── Run ───────────────────────────────────────────────────────
    signaling::run(config).await
}
