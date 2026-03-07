use clap::{Parser, Subcommand};
use graydr::cli::args::{CompileArgs, InitArgs, ValidateArgs};

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
    Version,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compile(args) => graydr::cli::compile::run_compile(args)?,
        Commands::Validate(args) => graydr::cli::validate::run_validate(args),
        Commands::Init(args) => graydr::cli::init::run_init(args)?,
        Commands::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}
