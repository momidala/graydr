use std::path::PathBuf;
use super::coord::ModuleCoord;

/// Returns the local cache path for a module coordinate.
/// Uses `dirs::cache_dir()`, joins `graydr/modules/{org}/{name}/{version}.gmod`.
pub fn cache_path(coord: &ModuleCoord) -> Option<PathBuf> {
    todo!()
}

/// Read the cached content for a module coordinate, if present.
pub fn read_cached(coord: &ModuleCoord) -> Option<String> {
    todo!()
}

/// Write content to the local cache for a module coordinate.
pub fn write_cache(coord: &ModuleCoord, content: &str) -> Result<(), std::io::Error> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_cache_path_structure() {
        // cache_path returns Some(path) where path ends with myorg/mymodule/1.2.3.gmod
        use semver::Version;
        let coord = ModuleCoord {
            org: "myorg".to_string(),
            name: "mymodule".to_string(),
            version: Version::new(1, 2, 3),
        };
        let path = cache_path(&coord).expect("cache_path should return Some");
        assert!(path.to_string_lossy().contains("myorg"));
        assert!(path.to_string_lossy().ends_with("myorg/mymodule/1.2.3.gmod"));
    }
}
