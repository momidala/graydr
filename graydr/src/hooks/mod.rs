use std::path::PathBuf;
use std::collections::HashMap;
use crate::registry::{RegistryClient, RegistryConfig, RegistryError, ModuleCoord, LifecycleState};

// ---------------------------------------------------------------------------
// ModuleResolveError
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ModuleResolveError {
    #[error("module '{module_name}' not found in any include path")]
    NotFound { module_name: String },
    #[error("I/O error reading module at '{path}': {source}")]
    Io { path: String, #[source] source: std::io::Error },
}

// ---------------------------------------------------------------------------
// ModuleResolver trait
// ---------------------------------------------------------------------------

pub trait ModuleResolver: Send + Sync {
    fn resolve(&self, module_name: &str, include_paths: &[PathBuf]) -> Result<String, ModuleResolveError>;
}

// ---------------------------------------------------------------------------
// FilesystemModuleResolver
// ---------------------------------------------------------------------------

pub struct FilesystemModuleResolver;

impl ModuleResolver for FilesystemModuleResolver {
    fn resolve(&self, module_name: &str, include_paths: &[PathBuf]) -> Result<String, ModuleResolveError> {
        for base in include_paths {
            let candidate = base.join(format!("{}.gmod", module_name));
            if candidate.exists() {
                return std::fs::read_to_string(&candidate).map_err(|e| ModuleResolveError::Io {
                    path: candidate.display().to_string(),
                    source: e,
                });
            }
        }
        Err(ModuleResolveError::NotFound { module_name: module_name.to_string() })
    }
}

// ---------------------------------------------------------------------------
// CompileSummary, ModuleUsage, ArmSelection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModuleUsage {
    pub resource_name: String,
    pub module_name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ArmSelection {
    pub resource_name: String,
    pub arm_keys: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CompileSummary {
    pub modules_used: Vec<ModuleUsage>,
    pub arms_selected: Vec<ArmSelection>,
    /// Sensitive values are already redacted by the compiler before populating this map.
    pub variable_values: HashMap<String, String>,
    pub template_path: String,
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// PostCompileHook trait
// ---------------------------------------------------------------------------

pub trait PostCompileHook: Send + Sync {
    fn on_compile_success(&self, summary: &CompileSummary);
}

// ---------------------------------------------------------------------------
// NoOpPostCompileHook
// ---------------------------------------------------------------------------

pub struct NoOpPostCompileHook;

impl PostCompileHook for NoOpPostCompileHook {
    fn on_compile_success(&self, _summary: &CompileSummary) {
        // Intentionally empty — CE default does nothing.
    }
}

// ---------------------------------------------------------------------------
// RegistryBackend trait
// ---------------------------------------------------------------------------

pub trait RegistryBackend: Send + Sync {
    fn fetch_module(&self, coord: &ModuleCoord) -> Result<String, RegistryError>;
    fn get_lifecycle(&self, coord: &ModuleCoord) -> Result<LifecycleState, RegistryError>;
}

// ---------------------------------------------------------------------------
// RegistryClient impl RegistryBackend
// ---------------------------------------------------------------------------

impl RegistryBackend for RegistryClient {
    fn fetch_module(&self, coord: &ModuleCoord) -> Result<String, RegistryError> {
        self.fetch_module(coord)
    }

    fn get_lifecycle(&self, coord: &ModuleCoord) -> Result<LifecycleState, RegistryError> {
        self.get_lifecycle(coord)
    }
}

// ---------------------------------------------------------------------------
// CompileHooks container
// ---------------------------------------------------------------------------

pub struct CompileHooks {
    pub module_resolver: Box<dyn ModuleResolver>,
    pub registry_backend: Option<Box<dyn RegistryBackend>>,
    pub post_compile: Box<dyn PostCompileHook>,
}

impl CompileHooks {
    /// Construct the CE default hooks.
    ///
    /// - `module_resolver`: `FilesystemModuleResolver`
    /// - `registry_backend`: `Some(RegistryClient)` when `GRAYDR_REGISTRY_URL` is non-empty; else `None`
    /// - `post_compile`: `NoOpPostCompileHook`
    pub fn default_ce() -> Self {
        let config = RegistryConfig::from_env();
        let registry_backend: Option<Box<dyn RegistryBackend>> = if config.base_url.is_empty() {
            None
        } else {
            Some(Box::new(RegistryClient::new(config)))
        };
        Self {
            module_resolver: Box::new(FilesystemModuleResolver),
            registry_backend,
            post_compile: Box::new(NoOpPostCompileHook),
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // --- Dummy resolver that always returns a stub string ---

    struct DummyResolver;

    impl ModuleResolver for DummyResolver {
        fn resolve(&self, _module_name: &str, _include_paths: &[PathBuf]) -> Result<String, ModuleResolveError> {
            Ok("module_content_stub".to_string())
        }
    }

    // --- Dummy registry backend that always returns ModuleNotFound ---

    struct DummyRegistryBackend;

    impl RegistryBackend for DummyRegistryBackend {
        fn fetch_module(&self, coord: &ModuleCoord) -> Result<String, RegistryError> {
            Err(RegistryError::ModuleNotFound { coordinate: coord.to_string() })
        }

        fn get_lifecycle(&self, _coord: &ModuleCoord) -> Result<LifecycleState, RegistryError> {
            Ok(LifecycleState::Active)
        }
    }

    #[test]
    fn dummy_resolver_returns_stub() {
        let resolver = DummyResolver;
        let result = resolver.resolve("anything", &[]);
        assert_eq!(result.unwrap(), "module_content_stub");
    }

    #[test]
    fn no_op_hook_does_not_panic() {
        let hook = NoOpPostCompileHook;
        let summary = CompileSummary {
            modules_used: vec![],
            arms_selected: vec![],
            variable_values: HashMap::new(),
            template_path: String::new(),
            project: None,
        };
        // Must not panic
        hook.on_compile_success(&summary);
    }

    #[test]
    fn dummy_registry_backend_returns_not_found() {
        use crate::registry::coord::ModuleCoord;
        let backend = DummyRegistryBackend;
        let coord = ModuleCoord::parse("org/mod@1.0.0").unwrap();
        let result = backend.fetch_module(&coord);
        assert!(
            matches!(result, Err(RegistryError::ModuleNotFound { .. })),
            "expected ModuleNotFound, got: {:?}",
            result
        );
    }

    #[test]
    fn filesystem_resolver_returns_not_found_when_no_paths() {
        let resolver = FilesystemModuleResolver;
        let result = resolver.resolve("nonexistent", &[]);
        assert!(
            matches!(result, Err(ModuleResolveError::NotFound { .. })),
            "expected NotFound, got: {:?}",
            result
        );
    }
}
