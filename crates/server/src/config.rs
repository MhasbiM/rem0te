/// Server configuration, loaded from CLI args and environment variables.
use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "rem0te-server")]
#[command(about = "Signaling & relay server for rem0te remote desktop")]
pub struct Config {
    /// Address to bind the HTTP server to.
    #[arg(long, env = "REM0TE_BIND", default_value = "0.0.0.0:8080")]
    pub bind: String,

    /// Shared secret token that agents must provide when registering.
    #[arg(long, env = "REM0TE_TOKEN", default_value = "changeme")]
    pub token: String,

    /// Path to serve static web frontend files from.
    /// If set, the server will serve the Vue SPA in addition to signaling.
    #[arg(long, env = "REM0TE_WEB_DIR")]
    pub web_dir: Option<String>,

    /// Heartbeat interval in seconds.
    #[arg(long, env = "REM0TE_HEARTBEAT_SECS", default_value = "15")]
    pub heartbeat_secs: u64,

    /// Agent is considered offline after this many missed heartbeats.
    #[arg(long, env = "REM0TE_HEARTBEAT_MISSED", default_value = "3")]
    pub heartbeat_missed: u64,
}
