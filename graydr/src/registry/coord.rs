use semver::Version;
use super::RegistryError;

/// A fully-qualified module coordinate of the form `org/name@version`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCoord {
    pub org: String,
    pub name: String,
    pub version: Version,
}

impl ModuleCoord {
    /// Parse a coordinate string of the form `org/name@semver`.
    pub fn parse(s: &str) -> Result<Self, RegistryError> {
        let (org_name, version_str) = s.split_once('@')
            .ok_or_else(|| RegistryError::MalformedCoordinate { raw: s.to_string() })?;
        let (org, name) = org_name.split_once('/')
            .ok_or_else(|| RegistryError::MalformedCoordinate { raw: s.to_string() })?;
        let version = Version::parse(version_str)
            .map_err(|_| RegistryError::InvalidSemVer {
                coordinate: s.to_string(),
                version: version_str.to_string(),
            })?;
        Ok(ModuleCoord { org: org.to_string(), name: name.to_string(), version })
    }
}

impl std::fmt::Display for ModuleCoord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.org, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_coordinate_parses() {
        let coord = ModuleCoord::parse("myorg/mymodule@1.2.3").unwrap();
        assert_eq!(coord.org, "myorg");
        assert_eq!(coord.name, "mymodule");
        assert_eq!(coord.version, Version::new(1, 2, 3));
    }

    #[test]
    fn test_prerelease_coordinate_parses() {
        let coord = ModuleCoord::parse("org/name@1.0.0-beta.1").unwrap();
        assert_eq!(coord.org, "org");
        assert_eq!(coord.name, "name");
        assert_eq!(coord.version.pre.as_str(), "beta.1");
    }

    #[test]
    fn test_missing_at_sign_is_error() {
        let result = ModuleCoord::parse("org/name");
        assert!(matches!(result, Err(RegistryError::MalformedCoordinate { .. })));
    }

    #[test]
    fn test_missing_slash_is_error() {
        let result = ModuleCoord::parse("orgname@1.0.0");
        assert!(matches!(result, Err(RegistryError::MalformedCoordinate { .. })));
    }

    #[test]
    fn test_bad_semver_is_error() {
        let result = ModuleCoord::parse("org/name@not-semver");
        assert!(matches!(result, Err(RegistryError::InvalidSemVer { .. })));
    }
}
