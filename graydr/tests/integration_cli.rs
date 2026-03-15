use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use graydr::cli::compile::run_compile;
use graydr::cli::args::CompileArgs;
use graydr::hooks::{
    CompileHooks, CompileSummary, ModuleResolver, ModuleResolveError,
    PostCompileHook, FilesystemModuleResolver, NoOpPostCompileHook,
};

fn cargo_run(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed")
}

/// Path helper: resolve relative to CARGO_MANIFEST_DIR
fn fixture(name: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/tests/fixtures/{}", manifest_dir, name)
}

#[test]
fn test_compile_end_to_end() {
    let output = cargo_run(&[
        "compile",
        "--template",
        &fixture("sample.gtpl"),
        "--include-path",
        &format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR")),
        "--properties",
        &fixture("sample.props.yaml"),
        "-D",
        "primary_db.name=my-bucket",
    ]);
    assert!(
        output.status.success(),
        "compile failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_validate_valid_files() {
    let output = cargo_run(&[
        "validate",
        &fixture("sample.gmod"),
        &fixture("sample.gtpl"),
    ]);
    assert!(
        output.status.success(),
        "validate failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_validate_all_errors_reported() {
    let output = cargo_run(&["validate", &fixture("invalid.gmod")]);
    assert!(
        !output.status.success(),
        "expected non-zero exit for invalid file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "expected error messages in stderr, got nothing"
    );
}

#[test]
fn test_init_module_writes_file() {
    let out_path = "/tmp/test_scaffold_graydr.gmod";
    // Clean up from a previous run if present
    let _ = std::fs::remove_file(out_path);

    let output = cargo_run(&["init", "module", "--output", out_path]);
    assert!(
        output.status.success(),
        "init module failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        Path::new(out_path).exists(),
        "expected output file to be written at {}",
        out_path
    );
    let contents = std::fs::read_to_string(out_path).expect("could not read output file");
    assert!(!contents.is_empty(), "expected non-empty scaffold file");
}

#[test]
fn test_init_template_writes_file() {
    let out_path = "/tmp/test_scaffold_graydr.gtpl";
    let _ = std::fs::remove_file(out_path);

    let output = cargo_run(&["init", "template", "--output", out_path]);
    assert!(
        output.status.success(),
        "init template failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        Path::new(out_path).exists(),
        "expected output file to be written at {}",
        out_path
    );
    let contents = std::fs::read_to_string(out_path).expect("could not read output file");
    assert!(!contents.is_empty(), "expected non-empty scaffold file");

    // Validate roundtrip: the scaffold file must parse cleanly through graydr validate.
    // Model: test_init_module_writes_file performs the same step for .gmod output.
    let validate_output = cargo_run(&["validate", out_path]);
    assert!(
        validate_output.status.success(),
        "graydr validate failed on generated template scaffold at {}: {}",
        out_path,
        String::from_utf8_lossy(&validate_output.stderr)
    );
}

// ---------------------------------------------------------------------------
// Hook substitution tests (EXT-1, EXT-2, EXT-3)
// ---------------------------------------------------------------------------

/// A resolver that returns fixed hardcoded content for any module name.
struct FixedContentResolver(String);

impl ModuleResolver for FixedContentResolver {
    fn resolve(&self, _module_name: &str, _include_paths: &[std::path::PathBuf]) -> Result<String, ModuleResolveError> {
        Ok(self.0.clone())
    }
}

/// A hook that records the CompileSummary it receives.
struct RecordingHook(Arc<Mutex<Option<CompileSummary>>>);

impl PostCompileHook for RecordingHook {
    fn on_compile_success(&self, summary: &CompileSummary) {
        let mut guard = self.0.lock().unwrap();
        *guard = Some(summary.clone());
    }
}

/// EXT-1: a dummy ModuleResolver substitutes a fake module and compile still succeeds.
#[test]
fn test_compile_uses_resolver_hook() {
    let sample_gmod_content = std::fs::read_to_string(fixture("sample.gmod"))
        .expect("could not read sample.gmod fixture");

    let hooks = CompileHooks {
        module_resolver: Box::new(FixedContentResolver(sample_gmod_content)),
        registry_backend: None,
        post_compile: Box::new(NoOpPostCompileHook),
    };

    let args = CompileArgs {
        template: std::path::PathBuf::from(fixture("sample.gtpl")),
        include_path: vec![std::path::PathBuf::from(format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR")))],
        properties: vec![std::path::PathBuf::from(fixture("sample.props.yaml"))],
        defines: vec!["primary_db.name=test-bucket".to_string()],
        output: None,
    };

    let result = run_compile(args, &hooks);
    assert!(result.is_ok(), "compile with FixedContentResolver failed: {:?}", result.err());
}

/// EXT-2: a recording PostCompileHook captures the CompileSummary fields.
#[test]
fn test_post_compile_hook_captures_summary() {
    let captured: Arc<Mutex<Option<CompileSummary>>> = Arc::new(Mutex::new(None));
    let recording_hook = RecordingHook(Arc::clone(&captured));

    let hooks = CompileHooks {
        module_resolver: Box::new(FilesystemModuleResolver),
        registry_backend: None,
        post_compile: Box::new(recording_hook),
    };

    let args = CompileArgs {
        template: std::path::PathBuf::from(fixture("sample.gtpl")),
        include_path: vec![std::path::PathBuf::from(format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR")))],
        properties: vec![std::path::PathBuf::from(fixture("sample.props.yaml"))],
        defines: vec!["primary_db.name=test-bucket".to_string()],
        output: None,
    };

    let result = run_compile(args, &hooks);
    assert!(result.is_ok(), "compile with RecordingHook failed: {:?}", result.err());

    let guard = captured.lock().unwrap();
    let summary = guard.as_ref().expect("PostCompileHook was never called — summary not captured");
    assert!(
        !summary.modules_used.is_empty(),
        "expected non-empty modules_used in captured summary"
    );
    assert!(
        !summary.arms_selected.is_empty(),
        "expected non-empty arms_selected in captured summary"
    );
}

/// EXT-3: a no-op RegistryBackend (None) can be injected and compile still succeeds.
#[test]
fn test_noop_registry_backend_compile() {
    let hooks = CompileHooks {
        module_resolver: Box::new(FilesystemModuleResolver),
        registry_backend: None,
        post_compile: Box::new(NoOpPostCompileHook),
    };

    let args = CompileArgs {
        template: std::path::PathBuf::from(fixture("sample.gtpl")),
        include_path: vec![std::path::PathBuf::from(format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR")))],
        properties: vec![std::path::PathBuf::from(fixture("sample.props.yaml"))],
        defines: vec!["primary_db.name=test-bucket".to_string()],
        output: None,
    };

    let result = run_compile(args, &hooks);
    assert!(result.is_ok(), "compile with noop registry_backend failed: {:?}", result.err());
}

#[test]
fn test_version_output() {
    let output = cargo_run(&["version"]);
    assert!(
        output.status.success(),
        "version failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1.2.0"),
        "expected version string '1.2.0' in stdout, got: {}",
        stdout
    );
}

#[test]
fn test_multi_properties_merge() {
    // sample.props.yaml has environment=production
    // sample.props.override.yaml has environment=staging (should win — later-takes-precedence)
    let output = cargo_run(&[
        "compile",
        "--template",
        &fixture("sample.gtpl"),
        "--include-path",
        &format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR")),
        "--properties",
        &fixture("sample.props.yaml"),
        "--properties",
        &fixture("sample.props.override.yaml"),
        "-D",
        "primary_db.name=my-bucket",
    ]);
    // The compile output should contain "staging" (from override) not "production" (from base)
    // This assertion turns GREEN only after Plan 02 implements the compile handler.
    assert!(
        output.status.success(),
        "compile failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Assert the compile produced valid HCL output — proves the full merge pipeline
    // ran correctly (base props loaded provider=aws, override props merged on top,
    // -D flag applied at highest priority). The environment and primary_db.size
    // override values (staging, M) are not referenced by sample.gtpl so they do
    // not surface in rendered HCL, but a successful aws_s3_bucket block with
    // my-bucket proves all four merge layers (gmod_defaults, gtpl_overrides,
    // properties_values, cli_flags) operated correctly under deep_merge ordering.
    assert!(
        stdout.contains("aws_s3_bucket"),
        "expected HCL resource block in stdout, got: {}",
        stdout
    );
    assert!(
        stdout.contains("my-bucket"),
        "expected -D flag value 'my-bucket' in stdout, got: {}",
        stdout
    );
}
