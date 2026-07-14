use std::env;

#[derive(Clone)]
pub struct ServerConfig {
    pub api_port: u16,
    pub signaling_port: u16,
    pub relay_port: u16,
    pub ws_port: u16,
    pub jwt_secret: String,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            api_port: env::var("REM0TE_API_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8080),
            signaling_port: env::var("REM0TE_SIGNALING_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(21116),
            relay_port: env::var("REM0TE_RELAY_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(21117),
            ws_port: env::var("REM0TE_WS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(21118),
            jwt_secret: env::var("REM0TE_JWT_SECRET")
                .unwrap_or_else(|_| "rem0te-dev-secret-change-me".to_string()),
        }
    }
}
