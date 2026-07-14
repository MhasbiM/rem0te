/// Client (remote agent) configuration.
use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "rem0te-client")]
#[command(about = "Remote desktop agent — runs on the machine to be controlled")]
pub struct Config {
    /// WebSocket URL of the signaling server.
    #[arg(
        long,
        env = "REM0TE_SERVER",
        default_value = "ws://localhost:8080/ws"
    )]
    pub server: String,

    /// Auth token shared with the server.
    #[arg(long, env = "REM0TE_TOKEN", default_value = "changeme")]
    pub token: String,

    /// Human-readable name for this machine.
    #[arg(long, env = "REM0TE_NAME")]
    pub name: Option<String>,

    /// Unique machine identifier (defaults to hostname).
    #[arg(long, env = "REM0TE_MACHINE_ID")]
    pub machine_id: Option<String>,

    /// Reconnect delay in seconds.
    #[arg(long, env = "REM0TE_RECONNECT_SECS", default_value = "5")]
    pub reconnect_secs: u64,

    /// Heartbeat interval in seconds.
    #[arg(long, env = "REM0TE_HEARTBEAT_SECS", default_value = "10")]
    pub heartbeat_secs: u64,
}
