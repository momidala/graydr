use super::{RegistryConfig, RegistryError};
use super::coord::ModuleCoord;
use super::lifecycle::LifecycleState;
use super::cache;

/// HTTP client for interacting with the graydr community registry.
pub struct RegistryClient {
    config: RegistryConfig,
    http: reqwest::blocking::Client,
}

impl RegistryClient {
    /// Create a new registry client with the given configuration.
    pub fn new(config: RegistryConfig) -> Self {
        RegistryClient {
            config,
            http: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client construction should not fail"),
        }
    }

    /// Publish a .gmod file to the registry.
    pub fn publish_module(
        &self,
        coord: &ModuleCoord,
        gmod_path: &std::path::Path,
    ) -> Result<(), RegistryError> {
        todo!()
    }

    /// Fetch module content from the registry.
    /// Checks local cache first; on cache miss performs
    /// GET /api/v1/modules/{org}/{name}/{version}/content.
    pub fn fetch_module(&self, coord: &ModuleCoord) -> Result<String, RegistryError> {
        // Cache-first: if already downloaded, return immediately
        if let Some(content) = cache::read_cached(coord) {
            return Ok(content);
        }
        // Network fetch
        let url = format!(
            "{}/api/v1/modules/{}/{}/{}/content",
            self.config.base_url, coord.org, coord.name, coord.version
        );
        let response = self
            .http
            .get(&url)
            .send()
            .map_err(|e| RegistryError::NetworkError {
                message: e.to_string(),
            })?
            .error_for_status()
            .map_err(|e| {
                if e.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                    RegistryError::ModuleNotFound {
                        coordinate: coord.to_string(),
                    }
                } else {
                    RegistryError::NetworkError {
                        message: e.to_string(),
                    }
                }
            })?;
        let content = response
            .text()
            .map_err(|e| RegistryError::NetworkError {
                message: e.to_string(),
            })?;
        // Write to cache (ignore cache write errors — non-fatal)
        let _ = cache::write_cache(coord, &content);
        Ok(content)
    }

    /// Retrieve the lifecycle state of a module.
    /// Performs GET /api/v1/modules/{org}/{name}/{version}/meta and
    /// parses the "lifecycle" JSON field.
    pub fn get_lifecycle(&self, coord: &ModuleCoord) -> Result<LifecycleState, RegistryError> {
        let url = format!(
            "{}/api/v1/modules/{}/{}/{}/meta",
            self.config.base_url, coord.org, coord.name, coord.version
        );
        let response = self
            .http
            .get(&url)
            .send()
            .map_err(|e| RegistryError::NetworkError {
                message: e.to_string(),
            })?
            .error_for_status()
            .map_err(|e| RegistryError::NetworkError {
                message: e.to_string(),
            })?;
        let meta: serde_json::Value = response
            .json()
            .map_err(|e| RegistryError::NetworkError {
                message: e.to_string(),
            })?;
        Ok(LifecycleState::from_str(
            meta["lifecycle"].as_str().unwrap_or("active"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_module_uses_cache() {
        // Pre-populate cache then call fetch_module with a URL that would 404
        let coord = ModuleCoord::parse("cachetest/mod@55.0.0").unwrap();
        cache::write_cache(&coord, "cached content").unwrap();
        let config = RegistryConfig {
            base_url: "http://127.0.0.1:1".to_string(),
            token: None,
        };
        let client = RegistryClient::new(config);
        let result = client.fetch_module(&coord).unwrap();
        assert_eq!(result, "cached content");
        // cleanup
        if let Some(p) = cache::cache_path(&coord) {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn test_retired_module_blocks_compile() {
        // get_lifecycle returning Retired should lead to a RetiredModule error
        // when the caller checks blocks_new_use().
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/api/v1/modules/retorg/retmod/3.0.0/meta")
            .with_status(200)
            .with_body(r#"{"lifecycle":"retired"}"#)
            .create();
        let config = RegistryConfig {
            base_url: server.url(),
            token: None,
        };
        let client = RegistryClient::new(config);
        let coord = ModuleCoord::parse("retorg/retmod@3.0.0").unwrap();
        let state = client.get_lifecycle(&coord).unwrap();
        assert!(
            state.blocks_new_use(),
            "Retired lifecycle must block new use"
        );
    }

    #[test]
    fn test_get_lifecycle_parses_json() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/api/v1/modules/org/name/1.0.0/meta")
            .with_status(200)
            .with_body(r#"{"lifecycle":"retired"}"#)
            .create();
        let config = RegistryConfig {
            base_url: server.url(),
            token: None,
        };
        let client = RegistryClient::new(config);
        let coord = ModuleCoord::parse("org/name@1.0.0").unwrap();
        let state = client.get_lifecycle(&coord).unwrap();
        assert_eq!(state, LifecycleState::Retired);
    }

    #[test]
    fn test_get_lifecycle_unknown_field_defaults_active() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/api/v1/modules/org/name/1.0.0/meta")
            .with_status(200)
            .with_body(r#"{"lifecycle":"unknown"}"#)
            .create();
        let config = RegistryConfig {
            base_url: server.url(),
            token: None,
        };
        let client = RegistryClient::new(config);
        let coord = ModuleCoord::parse("org/name@1.0.0").unwrap();
        let state = client.get_lifecycle(&coord).unwrap();
        assert_eq!(state, LifecycleState::Active);
    }

    #[test]
    fn test_fetch_module_writes_to_cache_after_download() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/api/v1/modules/dlorg/dlmod/2.0.0/content")
            .with_status(200)
            .with_body("module content from server")
            .create();
        let coord = ModuleCoord::parse("dlorg/dlmod@2.0.0").unwrap();
        // Ensure cache is clear
        if let Some(p) = cache::cache_path(&coord) {
            let _ = std::fs::remove_file(&p);
        }
        let config = RegistryConfig {
            base_url: server.url(),
            token: None,
        };
        let client = RegistryClient::new(config);
        let content = client.fetch_module(&coord).unwrap();
        assert_eq!(content, "module content from server");
        let cached = cache::read_cached(&coord).expect("should be cached after download");
        assert_eq!(cached, "module content from server");
        // cleanup
        if let Some(p) = cache::cache_path(&coord) {
            let _ = std::fs::remove_file(p);
        }
    }
}
