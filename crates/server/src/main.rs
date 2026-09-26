//! emusic-server CLI entry point.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use emusic_server::api::create_router;
use emusic_server::auth::generate_pairing_code;
use emusic_server::{AppState, Config, Database, RateLimiter, TokenManager};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "emusic-server",
    version,
    about = "Homelab music server for emusic"
)]
struct Cli {
    #[arg(short, long, default_value = "server.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the music server
    Run,
    /// Generate a 6-digit one-time device pairing code
    Pair {
        /// Optional explicit code (default: random 6 digits)
        #[arg(short, long)]
        code: Option<String>,
        /// Expiration time in seconds (default: 600)
        #[arg(short, long, default_value_t = 600)]
        ttl: i64,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cli = Cli::parse();

    let config = if cli.config.exists() {
        Config::load_from_file(&cli.config)?
    } else {
        tracing::info!("Config file not found, using default configuration");
        Config::default()
    };

    std::fs::create_dir_all(&config.server.data_dir)?;
    let db_path = config.server.data_dir.join("emusic-server.db");
    let db = Database::open(db_path)?;

    let secret_key_path = config.server.data_dir.join("secret.key");
    let (token_manager, _bytes) = if secret_key_path.exists() {
        let key_raw = std::fs::read(&secret_key_path)?;
        if key_raw.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&key_raw);
            (TokenManager::new(&arr)?, arr)
        } else {
            let (tm, bytes) = TokenManager::generate_random();
            std::fs::write(&secret_key_path, bytes)?;
            (tm, bytes)
        }
    } else {
        let (tm, bytes) = TokenManager::generate_random();
        std::fs::write(&secret_key_path, bytes)?;
        (tm, bytes)
    };

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Pair { code, ttl } => {
            let pairing_code = code.unwrap_or_else(generate_pairing_code);
            db.add_pairing_code(&pairing_code, ttl)?;
            println!("--------------------------------------------------");
            println!("  emusic-server One-Time Pairing Code: {}", pairing_code);
            println!("  Valid for {} seconds.", ttl);
            println!("--------------------------------------------------");
            Ok(())
        }
        Commands::Run => {
            tracing::info!("Scanning music libraries...");
            let scanned = emusic_server::scanner::scan_library(&config.library.paths, &db);
            tracing::info!("Scan completed: {} tracks indexed.", scanned);

            let state = Arc::new(AppState {
                db,
                token_manager,
                rate_limiter: RateLimiter::new(),
                config: config.clone(),
            });

            let app = create_router(state);
            let addr = format!("{}:{}", config.server.host, config.server.port);
            tracing::info!("emusic-server listening on {}", addr);

            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, app).await?;
            Ok(())
        }
    }
}
