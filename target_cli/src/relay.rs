use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct RelayClient {
    stream: TcpStream,
}

impl RelayClient {
    pub async fn connect_target(relay_addr: &str, session_id: &str) -> Result<Self> {
        let parts: Vec<&str> = relay_addr.rsplitn(2, ':').collect();
        let host = if parts.len() == 2 { parts[1] } else { relay_addr };
        let port: u16 = if parts.len() == 2 { parts[0].parse().unwrap_or(21117) } else { 21117 };

        let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await?;
        // Session ID must be exactly 36 bytes — pad with zeros if needed
        let sid_bytes = format!("{:0<36}", session_id).into_bytes();
        stream.write_all(&sid_bytes).await?;
        stream.write_all(&[1u8]).await?; // role 1 = target
        log::info!("Connected to relay session {}", session_id);
        Ok(Self { stream })
    }

    pub async fn send_frame(&mut self, data: &[u8]) -> Result<()> {
        let total_len = (1 + 4 + data.len()) as u32;
        let mut hdr = [0u8; 9];
        hdr[..4].copy_from_slice(&total_len.to_be_bytes());
        hdr[4] = 0; // MSG_FRAME
        hdr[5..9].copy_from_slice(&(data.len() as u32).to_be_bytes());
        self.stream.write_all(&hdr).await?;
        self.stream.write_all(data).await?;
        Ok(())
    }
}
