use std::fs;
use crate::cli::args::{InitArgs, InitKind};
use crate::cli::scaffold::{SCAFFOLD_MODULE, SCAFFOLD_TEMPLATE};

pub fn run_init(args: InitArgs) -> anyhow::Result<()> {
    match args.kind {
        InitKind::Module { output } => write_scaffold(SCAFFOLD_MODULE, output, "module.gmod"),
        InitKind::Template { output } => write_scaffold(SCAFFOLD_TEMPLATE, output, "template.gtpl"),
    }
}

fn write_scaffold(
    content: &str,
    output: Option<std::path::PathBuf>,
    _default_name: &str,
) -> anyhow::Result<()> {
    match output {
        Some(path) => {
            fs::write(&path, content)?;
            eprintln!("wrote scaffold to {}", path.display());
        }
        None => {
            print!("{}", content);
        }
    }
    Ok(())
}
