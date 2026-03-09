use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub storage_dir: PathBuf,
    pub max_upload_bytes: usize,
}

impl ServerConfig {
    pub fn new(port: u16, storage_dir: PathBuf) -> Self {
        Self {
            port,
            storage_dir,
            max_upload_bytes: 50 * 1024 * 1024, // 50 MB default
        }
    }
}
