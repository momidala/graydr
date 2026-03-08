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
        todo!()
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
        todo!()
    }

    /// Retrieve the lifecycle state of a module.
    /// Performs GET /api/v1/modules/{org}/{name}/{version}/meta and
    /// parses the "lifecycle" JSON field.
    pub fn get_lifecycle(&self, coord: &ModuleCoord) -> Result<LifecycleState, RegistryError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn test_fetch_module_uses_cache() {
        // If a cached file exists, no HTTP call should be made.
        // Use a tempdir to set up a fake cache entry and confirm
        // fetch_module returns the cached content without a network request.
        todo!()
    }

    #[test]
    #[ignore]
    fn test_retired_module_blocks_compile() {
        // get_lifecycle returning Retired should lead to a RetiredModule error
        // when the caller checks blocks_new_use().
        todo!()
    }
}
