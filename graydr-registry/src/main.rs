use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "graydr-registry", about = "graydr community registry server")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(long, default_value = "8080")]
        port: u16,
        #[arg(long, default_value = "./registry-data")]
        storage_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Serve { port, storage_dir } => {
            use std::sync::Arc;
            use graydr_registry::{AppState, config::ServerConfig, store::FilesystemStore, routes::build_router};

            tokio::fs::create_dir_all(&storage_dir).await?;

            // Count modules already on disk for startup log
            let module_count = count_modules(&storage_dir).await;

            let config = Arc::new(ServerConfig::new(port, storage_dir.clone()));
            let store = Arc::new(FilesystemStore::new(storage_dir));
            let state = Arc::new(AppState { store, config });
            let router = build_router(state);

            let addr = format!("0.0.0.0:{}", port);
            tracing::info!(port = port, modules = module_count, "graydr-registry starting");
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, router).await?;
        }
    }
    Ok(())
}

async fn count_modules(storage_dir: &std::path::Path) -> usize {
    let mut count = 0usize;
    if let Ok(mut orgs) = tokio::fs::read_dir(storage_dir).await {
        while let Ok(Some(org)) = orgs.next_entry().await {
            if let Ok(mut names) = tokio::fs::read_dir(org.path()).await {
                while let Ok(Some(name)) = names.next_entry().await {
                    if let Ok(mut versions) = tokio::fs::read_dir(name.path()).await {
                        while let Ok(Some(_)) = versions.next_entry().await {
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    count
}
