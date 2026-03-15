use crate::ast::module::ModuleDefinition;
use crate::lint::{LintDiagnostic, LintSeverity};

pub fn check(module: &ModuleDefinition) -> Vec<LintDiagnostic> {
    let meta = &module.metadata.value;
    let span = &module.metadata.span;
    let mut diags = Vec::new();

    // Only these five descriptive fields are checked. approval_required is a bool flag — omitted.
    let fields: &[(&'static str, bool)] = &[
        ("security_tier",          meta.security_tier.is_none()),
        ("compliance_frameworks",  meta.compliance_frameworks.is_none()),
        ("cost_tier",              meta.cost_tier.is_none()),
        ("data_classification",    meta.data_classification.is_none()),
        ("disaster_recovery_tier", meta.disaster_recovery_tier.is_none()),
    ];

    for (name, missing) in fields {
        if *missing {
            diags.push(LintDiagnostic {
                file: span.file.clone(),
                line: span.start_line,
                col: span.start_col,
                severity: LintSeverity::Warning,
                check: "missing-governance",
                message: format!("governance field '{}' is not set in metadata.governance", name),
            });
        }
    }
    diags
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
    fn test_missing_governance_fixture_triggers_five_warnings() {
        let diags = crate::lint::lint_file(&fixture_path("missing_governance.gmod"))
            .expect("lint_file failed");
        let gov: Vec<_> = diags.iter().filter(|d| d.check == "missing-governance").collect();
        assert_eq!(gov.len(), 5, "expected 5 missing-governance warnings, got: {:?}", gov);
    }

    #[test]
    fn test_clean_module_no_missing_governance() {
        let diags = crate::lint::lint_file(&fixture_path("clean.gmod"))
            .expect("lint_file failed");
        let gov: Vec<_> = diags.iter().filter(|d| d.check == "missing-governance").collect();
        assert!(gov.is_empty(), "clean.gmod should have no missing-governance warnings, got: {:?}", gov);
    }
}
