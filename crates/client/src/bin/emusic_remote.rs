//! `emusic-remote`: a small CLI client for `emusic-server`.
//!
//! Useful for headless testing and for scripted pulls on a homelab, and as a
//! reference for the app integration.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use emusic_client::auth::{
    generate_keypair, issue_refresh_proof, public_key_paserk, secret_key_paserk, token_fingerprint,
};
use emusic_client::cache::{safe_file_name, safe_id};
use emusic_client::{
    ClientError, CredentialStore, Credentials, RemoteClient, ServerEndpoint, unix_now,
};

/// Renew the token when it expires within this margin.
const REFRESH_MARGIN_SECS: i64 = 24 * 3600;

#[derive(Debug, Parser)]
#[command(
    name = "emusic-remote",
    version,
    about = "CLI client for emusic-server"
)]
struct Cli {
    /// Override the credential store directory.
    #[arg(long, global = true)]
    credentials_dir: Option<PathBuf>,
    /// Subcommand.
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Pair with a server using a one-time code.
    Pair {
        /// Server base URL.
        url: String,
        /// The one-time pairing code printed by `emusic-server pair`.
        #[arg(long)]
        code: String,
        /// Device name to register.
        #[arg(long, default_value = "emusic-remote")]
        name: String,
    },
    /// Show the server's health and this device's pairing status.
    Status {
        /// Server base URL.
        url: String,
    },
    /// List tracks from the server.
    List {
        /// Server base URL.
        url: String,
        /// Sync from this version instead of the stored one.
        #[arg(long)]
        since: Option<i64>,
        /// Maximum tracks to print.
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Download one track by id.
    Fetch {
        /// Server base URL.
        url: String,
        /// Track id.
        track_id: String,
        /// Destination file.
        out: PathBuf,
    },
    /// Sync and download all tracks into a directory.
    Pull {
        /// Server base URL.
        url: String,
        /// Destination directory.
        #[arg(long)]
        out: PathBuf,
        /// Maximum tracks to download (0 = all).
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// Download the HVSC `Songlengths.md5` text.
    Songlengths {
        /// Server base URL.
        url: String,
        /// Destination file.
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = credential_store(&cli)?;

    match cli.command {
        Command::Pair { url, code, name } => pair(&store, &url, &name, &code),
        Command::Status { url } => status(&store, &url),
        Command::List { url, since, limit } => list(&store, &url, since, limit),
        Command::Fetch { url, track_id, out } => fetch(&store, &url, &track_id, &out),
        Command::Pull { url, out, limit } => pull(&store, &url, &out, limit),
        Command::Songlengths { url, out } => songlengths(&store, &url, &out),
    }
}

fn credential_store(cli: &Cli) -> Result<CredentialStore> {
    match &cli.credentials_dir {
        Some(dir) => Ok(CredentialStore::with_dir(dir.clone())),
        None => CredentialStore::new().map_err(Into::into),
    }
}

fn endpoint(url: &str) -> Result<ServerEndpoint> {
    ServerEndpoint::new("server", url).map_err(Into::into)
}

fn pair(store: &CredentialStore, url: &str, name: &str, code: &str) -> Result<()> {
    let endpoint = endpoint(url)?;
    let client = RemoteClient::from_endpoint(&endpoint)?;
    let (secret, public) = generate_keypair()?;
    let response = client
        .pair(code, name, &public_key_paserk(&public)?)
        .context("pairing")?;
    let credentials = Credentials {
        device_id: response.device_id,
        device_name: response.device_name,
        secret: secret_key_paserk(&secret)?,
        token: response.auth_token,
        expires_at: response.expires_at,
        since_version: 0,
    };
    store.save(&endpoint.id, &credentials)?;
    println!(
        "paired with {} as {:?} (device {})",
        endpoint.url, credentials.device_name, credentials.device_id
    );
    Ok(())
}

fn status(store: &CredentialStore, url: &str) -> Result<()> {
    let endpoint = endpoint(url)?;
    let client = RemoteClient::from_endpoint(&endpoint)?;
    let health = client.health().context("health check")?;
    println!("{}: {}", endpoint.url, health.status);
    match store.load(&endpoint.id)? {
        Some(credentials) => {
            let remaining = credentials.expires_at.saturating_sub(unix_now()).max(0) / 3600;
            println!(
                "paired as {:?} (device {}, token expires in ~{remaining}h, version {})",
                credentials.device_name, credentials.device_id, credentials.since_version
            );
        }
        None => println!("not paired (run `emusic-remote pair <url> --code ...`)"),
    }
    Ok(())
}

fn list(store: &CredentialStore, url: &str, since: Option<i64>, limit: usize) -> Result<()> {
    let endpoint = endpoint(url)?;
    let client = RemoteClient::from_endpoint(&endpoint)?;
    let (credentials, token) = fresh_token(store, &client, &endpoint)?;
    let delta = client
        .sync(&token, since.unwrap_or(credentials.since_version))
        .context("sync")?;
    for track in delta.tracks.iter().take(limit) {
        let duration = track
            .duration_secs
            .map(|seconds| format!("{:.0}s", seconds))
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{}  {:<6}  {:<10}  {}",
            track.id,
            track.format,
            duration,
            track.display_title()
        );
    }
    println!(
        "version {} ({} changed, {} deleted)",
        delta.version,
        delta.tracks.len(),
        delta.deleted.len()
    );
    Ok(())
}

fn fetch(store: &CredentialStore, url: &str, track_id: &str, out: &PathBuf) -> Result<()> {
    let endpoint = endpoint(url)?;
    let client = RemoteClient::from_endpoint(&endpoint)?;
    let (_, token) = fresh_token(store, &client, &endpoint)?;
    let mut file =
        std::fs::File::create(out).with_context(|| format!("creating {}", out.display()))?;
    let bytes = client
        .download_to(&token, track_id, &mut file)
        .context("download")?;
    println!("wrote {bytes} bytes to {}", out.display());
    Ok(())
}

fn pull(store: &CredentialStore, url: &str, out: &PathBuf, limit: usize) -> Result<()> {
    let endpoint = endpoint(url)?;
    let client = RemoteClient::from_endpoint(&endpoint)?;
    let (mut credentials, token) = fresh_token(store, &client, &endpoint)?;
    let delta = client
        .sync(&token, credentials.since_version)
        .context("sync")?;
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;

    let mut downloaded = 0usize;
    let mut high_water = credentials.since_version;
    let mut truncated = false;
    for track in &delta.tracks {
        if limit > 0 && downloaded >= limit {
            truncated = true;
            break;
        }
        let name = safe_file_name(track)?;
        let destination = out.join(&name);
        high_water = high_water.max(track.sync_version);
        if let Ok(metadata) = std::fs::metadata(&destination)
            && track.file_size > 0
            && metadata.is_file()
            && metadata.len() == track.file_size
        {
            continue;
        }
        let temporary = destination.with_file_name(format!("{name}.part.{}", std::process::id()));
        let result = (|| -> Result<()> {
            let mut file = std::fs::File::create(&temporary)
                .with_context(|| format!("creating {}", temporary.display()))?;
            client
                .download_to(&token, &track.id, &mut file)
                .with_context(|| format!("download {}", track.id))?;
            file.sync_all()?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        if destination.exists() {
            std::fs::remove_file(&destination)?;
        }
        std::fs::rename(&temporary, &destination)?;
        downloaded += 1;
        println!("{} -> {}", track.display_title(), destination.display());
    }

    // Remove mirrors of tracks the server no longer has.
    for id in &delta.deleted {
        let Some(id) = safe_id(id) else {
            continue;
        };
        let prefix = format!("{id}.");
        let Ok(entries) = std::fs::read_dir(out) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) && !name.contains(".part.") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    // Only advance the version we actually drained; a truncated run keeps the
    // high-water mark so the skipped tracks reappear next time.
    credentials.since_version = if truncated { high_water } else { delta.version };
    store.save(&endpoint.id, &credentials)?;
    println!(
        "downloaded {downloaded} track(s); library version {}",
        delta.version
    );
    Ok(())
}

fn songlengths(store: &CredentialStore, url: &str, out: &PathBuf) -> Result<()> {
    let endpoint = endpoint(url)?;
    let client = RemoteClient::from_endpoint(&endpoint)?;
    let (_, token) = fresh_token(store, &client, &endpoint)?;
    let text = client.songlengths(&token).context("songlengths")?;
    std::fs::write(out, text).with_context(|| format!("writing {}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

/// Loads credentials and refreshes the token if it is close to expiry.
fn fresh_token(
    store: &CredentialStore,
    client: &RemoteClient,
    endpoint: &ServerEndpoint,
) -> Result<(Credentials, String)> {
    let mut credentials = store
        .load(&endpoint.id)?
        .ok_or(ClientError::NotPaired)
        .context("not paired; run `emusic-remote pair` first")?;
    if credentials.expiring_within(REFRESH_MARGIN_SECS, unix_now()) {
        let secret = credentials.secret_key()?;
        let fingerprint = token_fingerprint(&credentials.token);
        let proof = issue_refresh_proof(
            &secret,
            &fingerprint,
            Duration::from_secs(120),
            Duration::from_secs(30),
        )?;
        let response = client
            .refresh(&credentials.token, &proof)
            .context("refreshing token")?;
        credentials.token = response.auth_token;
        credentials.expires_at = response.expires_at;
        store.save(&endpoint.id, &credentials)?;
    }
    let token = credentials.token.clone();
    Ok((credentials, token))
}
