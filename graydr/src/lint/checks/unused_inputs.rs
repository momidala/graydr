use crate::ast::module::ModuleDefinition;
use crate::lint::{LintDiagnostic, LintSeverity};
use std::collections::HashSet;

pub fn check(module: &ModuleDefinition) -> Vec<LintDiagnostic> {
    // Collect all variable names referenced in:
    // (a) case arm code blocks (already scanned by parser)
    // (b) output template strings ($name syntax, via scan_variables)
    let referenced: HashSet<String> = module
        .generate
        .value
        .cases
        .iter()
        .flat_map(|c| c.value.arms.iter())
        .flat_map(|a| {
            // Variables referenced in code blocks (already scanned by parser)
            let code_vars = a.value.variables.iter().map(|v| v.value.name.clone());
            // Variables referenced in output template strings ($name syntax only)
            let output_vars = a.value.outputs.iter().flat_map(|o| {
                crate::parser::variable::scan_variables(
                    &o.value.template.value,
                    &o.value.template.span,
                )
                .into_iter()
                .map(|sv| sv.value.name)
            });
            code_vars.chain(output_vars)
        })
        .collect();

    module
        .interface
        .value
        .inputs
        .iter()
        .filter(|input| !referenced.contains(&input.value.name.value))
        .map(|input| LintDiagnostic {
            file: input.span.file.clone(),
            line: input.span.start_line,
            col: input.span.start_col,
            severity: LintSeverity::Warning,
            check: "unused-input",
            message: format!(
                "input '{}' is declared but never referenced in any case arm",
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
    fn test_unused_input_fixture_triggers_one_warning() {
        let diags = crate::lint::lint_file(&fixture_path("unused_input.gmod"))
            .expect("lint_file failed");
        let unused: Vec<_> = diags.iter().filter(|d| d.check == "unused-input").collect();
        assert_eq!(unused.len(), 1, "expected exactly 1 unused-input warning, got: {:?}", unused);
        assert!(unused[0].message.contains("region"), "expected warning about 'region'");
    }

    #[test]
    fn test_clean_module_no_unused_inputs() {
        let diags = crate::lint::lint_file(&fixture_path("clean.gmod"))
            .expect("lint_file failed");
        let unused: Vec<_> = diags.iter().filter(|d| d.check == "unused-input").collect();
        assert!(unused.is_empty(), "clean.gmod should have no unused-input warnings, got: {:?}", unused);
    }
}
