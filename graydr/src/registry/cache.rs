use std::path::PathBuf;
use super::coord::ModuleCoord;

/// Returns the local cache path for a module coordinate.
/// Uses `dirs::cache_dir()`, joins `graydr/modules/{org}/{name}/{version}.gmod`.
pub fn cache_path(coord: &ModuleCoord) -> Option<PathBuf> {
    let base = dirs::cache_dir()?;
    Some(base
        .join("graydr")
        .join("modules")
        .join(&coord.org)
        .join(&coord.name)
        .join(format!("{}.gmod", coord.version)))
}

/// Read the cached content for a module coordinate, if present.
pub fn read_cached(coord: &ModuleCoord) -> Option<String> {
    cache_path(coord).and_then(|p| std::fs::read_to_string(p).ok())
}

/// Write content to the local cache for a module coordinate.
pub fn write_cache(coord: &ModuleCoord, content: &str) -> Result<(), std::io::Error> {
    if let Some(p) = cache_path(coord) {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(p, content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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

    #[test]
    fn test_write_and_read_cache() {
        let coord = ModuleCoord::parse("testorg/testmod@99.0.0").unwrap();
        write_cache(&coord, "module content").expect("write_cache should not fail");
        let retrieved = read_cached(&coord).expect("read_cached should return content");
        assert_eq!(retrieved, "module content");
    }
}
