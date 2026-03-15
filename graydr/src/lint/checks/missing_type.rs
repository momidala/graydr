use crate::ast::module::ModuleDefinition;
use crate::lint::{LintDiagnostic, LintSeverity};

pub fn check(module: &ModuleDefinition) -> Vec<LintDiagnostic> {
    module
        .interface
        .value
        .inputs
        .iter()
        .filter(|input| !input.value.has_type)
        .map(|input| LintDiagnostic {
            file: input.span.file.clone(),
            line: input.span.start_line,
            col: input.span.start_col,
            severity: LintSeverity::Warning,    // Warning, not Error — reference modules lack types
            check: "missing-type",
            message: format!(
                "input '{}' has no type annotation (add `type = string` or `type = number`)",
                input.value.name.value
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
    fn test_missing_type_fixture_triggers_two_warnings() {
        let diags = crate::lint::lint_file(&fixture_path("missing_type.gmod"))
            .expect("lint_file failed");
        let typed: Vec<_> = diags.iter().filter(|d| d.check == "missing-type").collect();
        assert_eq!(typed.len(), 2, "expected 2 missing-type warnings, got: {:?}", typed);
    }

    #[test]
    fn test_clean_module_no_missing_type() {
        let diags = crate::lint::lint_file(&fixture_path("clean.gmod"))
            .expect("lint_file failed");
        let typed: Vec<_> = diags.iter().filter(|d| d.check == "missing-type").collect();
        assert!(typed.is_empty(), "clean.gmod should have no missing-type warnings");
    }

    #[test]
    fn test_clean_module_zero_total_diagnostics() {
        // Regression: clean.gmod passes all five checks
        let diags = crate::lint::lint_file(&fixture_path("clean.gmod"))
            .expect("lint_file failed");
        assert!(diags.is_empty(), "clean.gmod must produce zero diagnostics total, got: {:?}", diags);
    }
}
