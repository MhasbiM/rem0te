use serde::{Deserialize, Serialize};

/// Represents a file or directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
}

/// File transfer protocol message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FileTransferMessage {
    ListRequest {
        path: String,
    },
    ListResponse {
        path: String,
        entries: Vec<FileEntry>,
    },
    UploadRequest {
        file_path: String,
        file_size: u64,
        chunk_size: u32,
    },
    UploadChunk {
        file_path: String,
        offset: u64,
        data: Vec<u8>,
    },
    UploadComplete {
        file_path: String,
    },
    DownloadRequest {
        file_path: String,
        offset: u64,
        chunk_size: u32,
    },
    DownloadChunk {
        file_path: String,
        offset: u64,
        data: Vec<u8>,
        is_last: bool,
    },
    Error {
        message: String,
    },
}

/// Helper to read a directory and return file entries
pub fn list_directory(path: &std::path::Path) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    if !path.is_dir() {
        return Ok(entries);
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        });
    }

    // Sort: directories first, then by name
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

/// Chunk size for file transfers: 64KB
pub const CHUNK_SIZE: u32 = 65536;

/// Read a file in chunks for upload
pub async fn read_file_chunk(
    path: &str,
    offset: u64,
    chunk_size: u32,
) -> std::io::Result<(Vec<u8>, bool)> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let mut file = tokio::fs::File::open(path).await?;
    let file_size = file.metadata().await?.len();

    file.seek(std::io::SeekFrom::Start(offset)).await?;

    let mut buf = vec![0u8; chunk_size as usize];
    let n = file.read(&mut buf).await?;
    buf.truncate(n);

    let is_last = offset + n as u64 >= file_size;
    Ok((buf, is_last))
}
