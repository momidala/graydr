use clap::{Parser, Subcommand};
use graydr::cli::args::{CompileArgs, FmtArgs, InitArgs, LintArgs, PublishArgs, ValidateArgs};
use graydr::hooks::CompileHooks;

#[derive(Parser)]
#[command(name = "graydr", version, about = "IaC text preprocessor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Compile(CompileArgs),
    Validate(ValidateArgs),
    Init(InitArgs),
    Publish(PublishArgs),
    Fmt(FmtArgs),
    Lint(LintArgs),
    Version,
    /// Start the Language Server Protocol server over stdio
    Lsp,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compile(args) => graydr::cli::compile::run_compile(args, &CompileHooks::default_ce())?,
        Commands::Validate(args) => graydr::cli::validate::run_validate(args, &CompileHooks::default_ce()),
        Commands::Init(args) => graydr::cli::init::run_init(args)?,
        Commands::Publish(args) => graydr::cli::publish::run_publish(args)?,
        Commands::Fmt(args) => graydr::cli::fmt::run_fmt(args)?,
        Commands::Lint(args) => graydr::cli::lint::run_lint(args)?,
        Commands::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        Commands::Lsp => graydr::cli::lsp::run_lsp(),
    }
    Ok(())
}
