#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("module already exists at this coordinate; bump the version to publish a new release")]
    AlreadyExists,
    #[error("module not found")]
    NotFound,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
