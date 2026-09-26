//! Thin CLI entry point for `emusic-server`.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use emusic_server::{Config, server};

#[derive(Debug, Parser)]
#[command(name = "emusic-server", version, about)]
struct Cli {
    /// Path to `server.toml` (falls back to `EMUSIC_SERVER_CONFIG`).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Subcommand; defaults to `serve`.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the HTTP(S) server.
    Serve,
    /// Generate a one-time pairing code and print it.
    Pair {
        /// Validity of the code, in seconds.
        #[arg(long, default_value_t = 600)]
        ttl: u64,
    },
    /// List paired devices.
    Devices,
    /// Revoke a paired device by id.
    Revoke {
        /// The device id to revoke.
        device_id: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load_or_default(cli.config.as_deref()).context("loading configuration")?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .context("building async runtime")?;
            runtime
                .block_on(server::run(config))
                .context("running server")?;
        }
        Command::Pair { ttl } => {
            let code = server::print_pairing_code(&config, ttl).context("generating code")?;
            println!("{code}");
            println!("Pairing code valid for {ttl}s; enter it in the emusic client.");
        }
        Command::Devices => {
            for device in server::list_devices(&config).context("listing devices")? {
                let status = if device.is_revoked {
                    "revoked"
                } else {
                    "active"
                };
                let last_seen = device
                    .last_seen
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "never".to_string());
                println!(
                    "{}  {:<20}  {:<8}  last_seen={last_seen}",
                    device.id, device.name, status
                );
            }
        }
        Command::Revoke { device_id } => {
            if server::revoke_device(&config, &device_id).context("revoking device")? {
                println!("revoked {device_id}");
            } else {
                eprintln!("no such device: {device_id}");
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
