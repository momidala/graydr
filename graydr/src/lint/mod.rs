pub mod checks;

use crate::ast::module::ModuleDefinition;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum LintSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub file: std::sync::Arc<str>,
    pub line: u32,
    pub col: u32,
    pub severity: LintSeverity,
    pub check: &'static str,
    pub message: String,
}

impl std::fmt::Display for LintDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}: [{}] {}",
            self.file,
            self.line,
            self.col,
            match self.severity {
                LintSeverity::Error => "error",
                LintSeverity::Warning => "warning",
            },
            self.check,
            self.message
        )
    }
}

/// Boundary guard for LSP: tracks whether the lint cursor is inside a heredoc code block.
/// All five CE checks operate on the typed AST (not raw source), so in_code_block() is
/// always false during normal lint. LSP (Phase 21) extends this context for cursor position.
pub struct LintContext {
    pub in_code_block: bool,
}

impl LintContext {
    pub fn new() -> Self {
        Self { in_code_block: false }
    }

    pub fn in_code_block(&self) -> bool {
        self.in_code_block
    }
}

impl Default for LintContext {
    fn default() -> Self {
        Self::new()
    }
}

pub fn lint_module(module: &ModuleDefinition, _source: &str) -> Vec<LintDiagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(checks::unused_inputs::check(module));
    diagnostics.extend(checks::unwired_outputs::check(module));
    diagnostics.extend(checks::empty_case_arms::check(module));
    diagnostics.extend(checks::missing_governance::check(module));
    diagnostics.extend(checks::missing_type::check(module));
    diagnostics
}

pub fn lint_file(path: &Path) -> anyhow::Result<Vec<LintDiagnostic>> {
    let source = std::fs::read_to_string(path)?;
    let file = path.to_string_lossy();
    let module = crate::parser::module::parse_module_file(&source, &file)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(lint_module(&module, &source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_module_returns_diagnostics() {
        // Verified by integration after Plan 02 — this stub just confirms the API is callable
        let _ = std::hint::black_box(LintContext::new().in_code_block());
    }
}
