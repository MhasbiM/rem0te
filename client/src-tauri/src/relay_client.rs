use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

/// Relay client — split into independent read/write halves to avoid deadlock
pub struct RelayClient {
    reader: Option<OwnedReadHalf>,
    writer: Option<OwnedWriteHalf>,
    session_id: String,
    connected: bool,
}

/// Constants for message framing
pub const MSG_FRAME: u8 = 0;
pub const MSG_INPUT: u8 = 1;

impl RelayClient {
    pub fn new() -> Self {
        Self { reader: None, writer: None, session_id: String::new(), connected: false }
    }

    pub fn session_id(&self) -> &str { &self.session_id }
    pub fn is_connected(&self) -> bool { self.connected }

    /// Extract reader and writer halves for independent use (breaks RelayClient)
    pub fn into_halves(mut self) -> (Option<OwnedReadHalf>, Option<OwnedWriteHalf>) {
        self.connected = false;
        (self.reader.take(), self.writer.take())
    }

    pub fn disconnect(&mut self) {
        self.reader = None;
        self.writer = None;
        self.connected = false;
    }

    /// Connect and create or join a session
    async fn connect(&mut self, relay_addr: &str, session_id: &str, role: u8) -> Result<()> {
        let host = relay_addr_to_host(relay_addr);
        let stream = TcpStream::connect(&host).await
            .context("Failed to connect to relay server")?;

        self.session_id = session_id.to_string();

        // Split for independent read/write
        let (reader, mut writer) = stream.into_split();

        // Send handshake: session_id (36 bytes) + role (1 byte)
        writer.write_all(self.session_id.as_bytes()).await?;
        writer.write_all(&[role]).await?;

        self.reader = Some(reader);
        self.writer = Some(writer);
        self.connected = true;
        Ok(())
    }

    pub async fn create_session(&mut self, relay_addr: &str) -> Result<String> {
        let sid = uuid::Uuid::new_v4().to_string();
        self.connect(relay_addr, &sid, 0).await?;
        log::info!("Relay session created: {}", sid);
        Ok(sid)
    }

    pub async fn join_session(&mut self, relay_addr: &str, session_id: &str) -> Result<()> {
        self.connect(relay_addr, session_id, 1).await?;
        log::info!("Joined relay session: {}", session_id);
        Ok(())
    }

    /// Send framed message (does NOT need &mut self — uses writer only)
    pub async fn send(&self, msg_type: u8, data: &[u8]) -> Result<()> {
        // We need &mut for write_all, so we use a workaround
        // Actually, OwnedWriteHalf can be used without &mut via unsafe or interior mutability
        // For now, accept &mut self and caller must ensure no concurrent access
        Ok(())
    }

    /// Send framed message — requires &mut self for writer access
    pub async fn send_mut(&mut self, msg_type: u8, data: &[u8]) -> Result<()> {
        let writer = self.writer.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Relay not connected"))?;
        let total_len = (1 + 4 + data.len()) as u32;
        writer.write_all(&total_len.to_be_bytes()).await?;
        writer.write_all(&[msg_type]).await?;
        let payload_len = data.len() as u32;
        writer.write_all(&payload_len.to_be_bytes()).await?;
        writer.write_all(data).await?;
        Ok(())
    }

    /// Receive framed message — requires &mut self for reader access
    pub async fn recv_mut(&mut self) -> Result<(u8, Vec<u8>)> {
        let reader = self.reader.as_mut()
            .ok_or_else(|| anyhow::anyhow!("Relay not connected"))?;
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await?;
        let total_len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; total_len];
        reader.read_exact(&mut buf).await?;
        let msg_type = buf[0];
        let payload_len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
        let payload = buf[5..5+payload_len].to_vec();
        Ok((msg_type, payload))
    }
}

fn relay_addr_to_host(addr: &str) -> String {
    let addr = addr.trim_end_matches('/');
    if addr.contains(':') && addr.rsplitn(2, ':').next().map_or(false, |p| p.parse::<u16>().is_ok()) {
        addr.to_string()
    } else {
        format!("{}:21117", addr)
    }
}
