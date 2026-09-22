//! Command-line interface (#11): mock mode, files to open/enqueue at
//! startup (also what a secondary launch forwards to the running instance),
//! and one-shot file-association (un)registration.

use std::path::PathBuf;

use clap::Parser;

/// emusic: a MusicBee-inspired music player.
#[derive(Parser, Debug, Default)]
#[command(name = "emusic")]
pub struct Cli {
    /// Run against deterministic fake data instead of a real library/player
    /// backend (no BASS, no database). Useful for development and for
    /// `emusic-shot` screenshots.
    #[arg(long)]
    pub mock: bool,

    /// Add `files` to the running instance's queue instead of replacing it
    /// and starting playback.
    #[arg(long)]
    pub enqueue: bool,

    /// Register emusic as a handler for the audio file types it can play
    /// (per-user, no admin rights), then exit without starting the UI.
    #[arg(long = "register-associations")]
    pub register_associations: bool,

    /// Remove any emusic file associations previously registered, then exit
    /// without starting the UI.
    #[arg(long)]
    pub unregister: bool,

    /// Audio files to open (or enqueue, with `--enqueue`). If another
    /// instance is already running, these are forwarded to it instead of
    /// opening a second window.
    pub files: Vec<PathBuf>,
}
