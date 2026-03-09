pub mod cache;
pub mod client;
pub mod coord;
pub mod lifecycle;

pub use client::RegistryClient;
pub use coord::ModuleCoord;
pub use lifecycle::LifecycleState;

/// All errors that can arise from registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("malformed module coordinate '{raw}': expected 'org/name@version'")]
    MalformedCoordinate { raw: String },
    #[error("invalid SemVer version '{version}' in coordinate '{coordinate}'")]
    InvalidSemVer { coordinate: String, version: String },
    #[error("module '{coordinate}' is retired and cannot be used; check for a newer active version")]
    RetiredModule { coordinate: String },
    #[error("registry network error: {message}")]
    NetworkError { message: String },
    #[error("registry authentication required; set GRAYDR_REGISTRY_TOKEN env var")]
    AuthRequired,
    #[error("module not found in registry: {coordinate}")]
    ModuleNotFound { coordinate: String },
    #[error("cache I/O error: {0}")]
    CacheIo(#[from] std::io::Error),
}

/// Configuration for connecting to a registry.
pub struct RegistryConfig {
    pub base_url: String,
    pub token: Option<String>,
}

impl RegistryConfig {
    /// Build config from environment variables.
    /// Reads `GRAYDR_REGISTRY_URL` (default: empty string) and
    /// `GRAYDR_REGISTRY_TOKEN` (optional).
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("GRAYDR_REGISTRY_URL").unwrap_or_default(),
            token: std::env::var("GRAYDR_REGISTRY_TOKEN").ok(),
        }
    }
}
