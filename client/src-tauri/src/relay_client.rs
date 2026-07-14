use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Relay client for establishing a data channel between peers
pub struct RelayClient {
    stream: Option<TcpStream>,
    session_id: String,
    connected: bool,
}

impl RelayClient {
    pub fn new() -> Self {
        Self { stream: None, session_id: String::new(), connected: false }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Create a relay session (called by the initiator/viewer)
    pub async fn create_session(&mut self, relay_addr: &str) -> Result<String> {
        let addr = relay_addr.trim_end_matches('/');
        // Default relay port is 21117
        let relay_host = if addr.contains(':') {
            let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
            if parts.len() == 2 && parts[0].parse::<u16>().is_ok() {
                // Already has port
                addr.to_string()
            } else {
                format!("{}:21117", addr)
            }
        } else {
            format!("{}:21117", addr)
        };

        log::info!("Connecting to relay at {}", relay_host);

        let mut stream = TcpStream::connect(&relay_host)
            .await
            .context("Failed to connect to relay server")?;

        // Generate session ID
        self.session_id = uuid::Uuid::new_v4().to_string();

        // Send session ID (36 bytes) + role (0 = initiator)
        let session_bytes = self.session_id.as_bytes();
        stream.write_all(session_bytes).await?;
        stream.write_all(&[0u8]).await?;

        self.stream = Some(stream);
        self.connected = true;

        log::info!("Relay session created: {}", self.session_id);
        Ok(self.session_id.clone())
    }

    /// Join an existing relay session (called by the target)
    pub async fn join_session(&mut self, relay_addr: &str, session_id: &str) -> Result<()> {
        let addr = relay_addr.trim_end_matches('/');
        let relay_host = if addr.contains(':') {
            let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
            if parts.len() == 2 && parts[0].parse::<u16>().is_ok() {
                addr.to_string()
            } else {
                format!("{}:21117", addr)
            }
        } else {
            format!("{}:21117", addr)
        };

        log::info!("Joining relay session {} at {}", session_id, relay_host);

        let mut stream = TcpStream::connect(&relay_host)
            .await
            .context("Failed to connect to relay server")?;

        self.session_id = session_id.to_string();

        // Send session ID + role (1 = joiner)
        let session_bytes = self.session_id.as_bytes();
        stream.write_all(session_bytes).await?;
        stream.write_all(&[1u8]).await?;

        self.stream = Some(stream);
        self.connected = true;

        log::info!("Joined relay session: {}", self.session_id);
        Ok(())
    }

    /// Send data through the relay
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Relay not connected"))?;
        // Prefix with 4-byte length
        let len = data.len() as u32;
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(data).await?;
        Ok(())
    }

    /// Receive data from the relay
    pub async fn recv(&mut self) -> Result<Vec<u8>> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Relay not connected"))?;
        // Read 4-byte length prefix
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        // Read payload
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;
        Ok(data)
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
        self.connected = false;
    }
}
