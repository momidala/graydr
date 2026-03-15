use crate::cli::args::LintArgs;
use crate::lint::{LintSeverity, lint_file};

pub fn run_lint(args: LintArgs) -> anyhow::Result<()> {
    let mut has_error = false;
    let mut has_warning = false;

    for path in &args.files {
        match lint_file(path) {
            Ok(diagnostics) => {
                for d in &diagnostics {
                    eprintln!("{}", d);
                    match d.severity {
                        LintSeverity::Error => has_error = true,
                        LintSeverity::Warning => has_warning = true,
                    }
                }
            }
            Err(e) => {
                eprintln!("error: {}: {}", path.display(), e);
                has_error = true;
            }
        }
    }

    if has_error || (args.strict && has_warning) {
        std::process::exit(1);
    }
    Ok(())
}
