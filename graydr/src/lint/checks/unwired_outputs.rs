use crate::ast::module::ModuleDefinition;
use crate::lint::{LintDiagnostic, LintSeverity};
use std::collections::HashSet;

pub fn check(module: &ModuleDefinition) -> Vec<LintDiagnostic> {
    // Collect all output names that appear in at least one arm's outputs block
    let wired: HashSet<String> = module
        .generate
        .value
        .cases
        .iter()
        .flat_map(|c| c.value.arms.iter())
        .flat_map(|a| a.value.outputs.iter())
        .map(|o| o.value.name.value.clone())
        .collect();

    module
        .interface
        .value
        .outputs
        .iter()
        .filter(|output| !wired.contains(&output.value.name.value))
        .map(|output| LintDiagnostic {
            file: output.span.file.clone(),
            line: output.span.start_line,
            col: output.span.start_col,
            severity: LintSeverity::Warning,
            check: "unwired-output",
            message: format!(
                "output '{}' is declared in interface but never wired in any case arm",
                output.value.name.value
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/lint")
            .join(name)
    }

    #[test]
    fn test_unwired_output_fixture_triggers_one_warning() {
        let diags = crate::lint::lint_file(&fixture_path("unwired_output.gmod"))
            .expect("lint_file failed");
        let unwired: Vec<_> = diags.iter().filter(|d| d.check == "unwired-output").collect();
        assert_eq!(unwired.len(), 1, "expected 1 unwired-output warning, got: {:?}", unwired);
        assert!(unwired[0].message.contains("port"), "expected warning about 'port'");
    }

    #[test]
    fn test_clean_module_no_unwired_outputs() {
        let diags = crate::lint::lint_file(&fixture_path("clean.gmod"))
            .expect("lint_file failed");
        let unwired: Vec<_> = diags.iter().filter(|d| d.check == "unwired-output").collect();
        assert!(unwired.is_empty(), "clean.gmod should have no unwired-output warnings");
    }
}
