use crate::ast::module::ModuleDefinition;
use crate::lint::{LintDiagnostic, LintSeverity};

pub fn check(module: &ModuleDefinition) -> Vec<LintDiagnostic> {
    module
        .generate
        .value
        .cases
        .iter()
        .flat_map(|c| c.value.arms.iter())
        .filter(|arm| arm.value.code.value.trim().is_empty())
        .map(|arm| LintDiagnostic {
            file: arm.span.file.clone(),
            line: arm.span.start_line,
            col: arm.span.start_col,
            severity: LintSeverity::Warning,
            check: "empty-case-arm",
            message: format!(
                "case arm '{}' has an empty code block",
                arm.value.keys.first().map(|k| k.value.as_str()).unwrap_or("?")
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
    fn test_empty_arm_fixture_triggers_one_warning() {
        let diags = crate::lint::lint_file(&fixture_path("empty_arm.gmod"))
            .expect("lint_file failed");
        let empty: Vec<_> = diags.iter().filter(|d| d.check == "empty-case-arm").collect();
        assert_eq!(empty.len(), 1, "expected 1 empty-case-arm warning, got: {:?}", empty);
    }

    #[test]
    fn test_clean_module_no_empty_arms() {
        let diags = crate::lint::lint_file(&fixture_path("clean.gmod"))
            .expect("lint_file failed");
        let empty: Vec<_> = diags.iter().filter(|d| d.check == "empty-case-arm").collect();
        assert!(empty.is_empty(), "clean.gmod should have no empty-case-arm warnings");
    }
}
