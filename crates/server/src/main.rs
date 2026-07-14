//! rem0te signaling server.
//!
//! Handles WebSocket signaling between remote agents and web clients,
//! relays WebRTC SDP/ICE messages, and optionally serves the Vue SPA.

mod config;
mod routes;
mod signaling;

use std::net::SocketAddr;

use clap::Parser;
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use config::Config;
use routes::{build_router, AppState};
use signaling::SignalingHub;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Logging ───────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rem0te_server=debug,info")),
        )
        .init();

    // ── Config ────────────────────────────────────────────────────
    let config = Config::parse();

    info!("🚀 rem0te signaling server starting");
    info!("   bind: {}", config.bind);
    info!("   web dir: {:?}", config.web_dir);

    // ── Signaling hub ─────────────────────────────────────────────
    let hub = SignalingHub::new(config.token.clone());

    // Optional: background task to check heartbeats
    let heartbeat_hub = hub.clone();
    let max_missed = config.heartbeat_missed;
    let interval_secs = config.heartbeat_secs;
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        loop {
            tick.tick().await;
            let machines = heartbeat_hub.list_machines();
            for machine in &machines {
                if heartbeat_hub.agent_missed_heartbeat(&machine.machine_id, max_missed) {
                    error!(machine_id = %machine.machine_id, "agent timed out, removing");
                    heartbeat_hub.unregister_agent(&machine.machine_id);
                }
            }
        }
    });

    // ── Router ────────────────────────────────────────────────────
    let state = AppState {
        hub,
        heartbeat_secs: config.heartbeat_secs,
        heartbeat_missed: config.heartbeat_missed,
    };

    let router = build_router(state, config.web_dir.clone());

    // ── Bind ─────────────────────────────────────────────────────
    let addr: SocketAddr = config.bind.parse()?;
    let listener = TcpListener::bind(addr).await?;

    info!("✅ server listening on http://{}", addr);

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
