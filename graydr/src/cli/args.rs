use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Arguments for the `compile` subcommand.
#[derive(Parser, Debug)]
pub struct CompileArgs {
    #[arg(long, help = "Path to .gtpl template file")]
    pub template: PathBuf,
    #[arg(long, help = "Directory to search for .gmod files and .gfrag fragments (repeatable)")]
    pub include_path: Vec<PathBuf>,
    #[arg(short = 'D', value_name = "KEY=VALUE", help = "Override variable (repeatable)")]
    pub defines: Vec<String>,
    #[arg(long, value_name = "FILE", help = "Properties file (repeatable, later takes precedence)")]
    pub properties: Vec<PathBuf>,
    #[arg(long, value_name = "FILE", help = "Write compiled output to file (default: stdout)")]
    pub output: Option<PathBuf>,
}

/// Arguments for the `validate` subcommand.
#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// .gmod or .gtpl files to validate
    pub files: Vec<PathBuf>,
}

/// Arguments for the `init` subcommand.
#[derive(Parser, Debug)]
pub struct InitArgs {
    #[command(subcommand)]
    pub kind: InitKind,
}

/// Sub-subcommand for `init`.
#[derive(Subcommand, Debug)]
pub enum InitKind {
    Module {
        #[arg(long, help = "Write scaffold to file (default: stdout)")]
        output: Option<PathBuf>,
    },
    Template {
        #[arg(long, help = "Write scaffold to file (default: stdout)")]
        output: Option<PathBuf>,
    },
}

/// Arguments for the `publish` subcommand.
#[derive(Parser, Debug)]
pub struct PublishArgs {
    #[arg(long, value_name = "FILE", help = "Path to .gmod file to publish")]
    pub module: PathBuf,
    #[arg(long, value_name = "URL", help = "Registry base URL (overrides GRAYDR_REGISTRY_URL)")]
    pub registry: Option<String>,
}

/// Arguments for the `fmt` subcommand.
#[derive(Parser, Debug)]
pub struct FmtArgs {
    /// .gmod, .gtpl, or .gfrag files to format
    pub files: Vec<PathBuf>,
    #[arg(long, help = "Check if files are canonical without rewriting; exits non-zero if any file would change")]
    pub check: bool,
}

/// Arguments for the `lint` subcommand.
#[derive(Parser, Debug)]
pub struct LintArgs {
    /// .gmod files to lint (accepts multiple files)
    pub files: Vec<PathBuf>,
    /// Exit non-zero for warnings in addition to errors (CI enforcement mode)
    #[arg(long, help = "Exit non-zero for warnings in addition to errors")]
    pub strict: bool,
}
