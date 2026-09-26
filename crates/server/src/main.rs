use std::path::PathBuf;
use std::sync::Arc;
use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;

mod config;
mod db;
mod util;
mod scanner;
mod api;

#[cfg(test)]
mod tests;

use config::AppConfig;
use db::DbStore;
use util::security::{TokenManager, generate_pairing_code};
use scanner::LibraryScanner;
use api::{AppState, create_router};

#[derive(Parser)]
#[command(name = "emusic-server")]
#[command(about = "Homelab music server for emusic", long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the music server
    Serve,
    /// Generate a 6-digit one-time device pairing code
    Pair {
        #[arg(short, long, default_value = "300")]
        ttl: u64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let cli = Cli::parse();
    let config = AppConfig::load_or_default(cli.config.as_deref());

    let db = Arc::new(DbStore::new(&config.server.data_dir)?);
    let secret = if let Ok(env_secret) = std::env::var("EMUSIC_SECRET") {
        env_secret.into_bytes()
    } else {
        db.get_or_create_secret_key()?
    };

    let token_mgr = Arc::new(TokenManager::new(secret));

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Pair { ttl } => {
            let code = generate_pairing_code();
            db.save_pairing_code(&code, ttl)?;
            println!("========================================");
            println!("  emusic-server Device Pairing Code");
            println!("========================================");
            println!("  PAIRING CODE: {}", code);
            println!("  Expires in:   {} seconds", ttl);
            println!("========================================");
        }
        Commands::Serve => {
            info!("Starting emusic-server...");

            let scanner_db = Arc::clone(&db);
            let scan_paths = config.library.paths.clone();
            tokio::task::spawn_blocking(move || {
                let scanner = LibraryScanner::new(scanner_db);
                scanner.scan(&scan_paths);
            });

            let state = Arc::new(AppState {
                db,
                token_mgr,
                config: config.clone(),
            });

            let app = create_router(state);
            let addr = format!("{}:{}", config.server.host, config.server.port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            info!("Listening on http://{}", addr);

            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn pairing_code_is_six_digits() {
        let code = generate_pairing_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }
}
