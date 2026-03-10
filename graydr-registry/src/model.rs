use semver::Version;

/// A fully-qualified module coordinate (org/name@version).
/// Re-implemented here to keep graydr-registry independent of the graydr compiler crate.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCoord {
    pub org: String,
    pub name: String,
    pub version: Version,
}

/// Lifecycle state of a published module.
/// CRITICAL: Must serialize as lowercase ("active", not "Active") — client from_str expects lowercase.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleState {
    Beta,
    Active,
    Deprecated,
    Retired,
}

impl Default for LifecycleState {
    fn default() -> Self { Self::Active }
}

/// JSON shape for the /meta endpoint response and meta.json sidecar file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModuleMeta {
    pub org: String,
    pub name: String,
    pub version: String,
    pub lifecycle: LifecycleState,
    pub published_at: String,
}

/// JSON shape for a single entry in the GET /versions response.
#[derive(Debug, serde::Serialize)]
pub struct VersionEntry {
    pub version: String,
    pub lifecycle: LifecycleState,
    pub published_at: String,
}
