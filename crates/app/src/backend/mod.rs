//! Chooses and wires up the real backends (#11, #54): the BASS-backed
//! [`emusic_player::Player`] via [`player_adapter::PlayerAdapter`], the
//! SQLite-backed library via [`library::LibraryBackend`], and
//! single-instance/IPC via `winshell` ([`ipc`]).
//!
//! `--mock` continues to use the in-memory fakes from [`crate::mock`].

pub mod ipc;
pub mod library;
pub mod player_adapter;
pub mod smtc;
pub mod thumbbar;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::{info, warn};

use crate::library_api::LibraryDataSource;
use crate::mock;
use crate::player_api::{
    ModuleInfo, NowPlayingInfo, PlaybackStatus, PlayerApi, QueueEntry, RepeatMode,
};
use library::LibraryBackend;
use player_adapter::PlayerAdapter;

/// The library/player pair the app runs with, plus a startup notice (e.g.
/// "no audio device") the shell should surface instead of silently limping
/// along.
pub struct Backends {
    pub library: Box<dyn LibraryDataSource>,
    pub player: Box<dyn PlayerApi>,
    pub notice: Option<String>,
}

/// Builds the backends for this run: fakes for `--mock`, otherwise the real
/// library store + BASS-backed player. If BASS can't be loaded (missing DLLs,
/// no output device, ...) the app still starts, with an inert player and a
/// notice describing why, rather than crashing (#11).
pub fn build(mock: bool) -> Backends {
    if mock {
        let library = mock::MockLibrary::new();
        // A fixed, arbitrary index into the deterministically seeded mock
        // library, just so "now playing" points at a real track (and thus
        // highlights a real row in the track table) instead of made-up
        // metadata that never matches anything.
        let player = mock::MockPlayer::playing_demo(&library.tracks()[0]);
        return Backends {
            library: Box::new(library),
            player: Box::new(player),
            notice: None,
        };
    }

    // BASS is initialized before the library backend so one instance can be
    // shared: a clone goes to the player for playback, another to the
    // library backend so the scanner can read tracker module tags (#138).
    // When BASS fails to init, both fall back to their BASS-less behavior
    // (no playback; modules counted but skipped during scans).
    let (bass, notice) = match init_bass() {
        Ok(bass) => {
            info!("BASS initialized");
            load_bass_plugins(&bass);
            (Some(Arc::new(bass)), None)
        }
        Err(err) => {
            warn!(%err, "audio backend unavailable; starting without playback");
            let notice = format!(
                "Audio unavailable: {err} (looked in {}; set EMUSIC_BASS_DIR to override)",
                bass_dir().display()
            );
            (None, Some(notice))
        }
    };

    let library = LibraryBackend::new(bass.clone());
    let play_record_tx = library.play_record_tx();

    let player: Box<dyn PlayerApi> = match bass {
        Some(bass) => {
            let backend = Arc::new(emusic_player::BassBackend::new(bass));
            let player =
                PlayerAdapter::new(emusic_player::Player::new(backend), Some(play_record_tx));
            Box::new(player)
        }
        None => Box::new(UnavailablePlayer),
    };

    Backends {
        library: Box::new(library),
        player,
        notice,
    }
}

/// Environment variable overriding where BASS DLLs are loaded from; matches
/// `bass::ffi::loader::BASS_DIR_ENV` (private to that crate, so the name is
/// duplicated here rather than imported).
const BASS_DIR_ENV: &str = "EMUSIC_BASS_DIR";

/// Directory BASS DLLs are loaded from: `$EMUSIC_BASS_DIR` if set, otherwise
/// a `bass` folder next to the running executable. Mirrors
/// `bass::ffi::loader::bass_dir` (private to that crate); duplicated here
/// only for the plugin-loading and log-message paths, since `Bass::init`
/// resolves `bass.dll` itself.
fn bass_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(BASS_DIR_ENV) {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|dir| dir.join("bass"))
        .unwrap_or_else(|| PathBuf::from("bass"))
}

fn init_bass() -> Result<bass::Bass, bass::BassError> {
    bass::Bass::init(-1, 44100)
}

fn load_bass_plugins(bass: &bass::Bass) {
    for result in bass.load_plugins(bass_dir()) {
        if let Err(err) = result.result {
            warn!(plugin = %result.path.display(), %err, "BASS plugin failed to load");
        }
    }
}

/// Inert [`PlayerApi`] used when the real audio backend couldn't start (see
/// [`build`]'s `notice`). Every command is a no-op so the shell keeps
/// running and displaying its `notice` instead of crashing or panicking.
struct UnavailablePlayer;

impl PlayerApi for UnavailablePlayer {
    fn tick(&mut self, _dt: std::time::Duration) {}
    fn status(&self) -> PlaybackStatus {
        PlaybackStatus::Stopped
    }
    fn now_playing(&self) -> Option<&NowPlayingInfo> {
        None
    }
    fn position(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
    fn duration(&self) -> Option<std::time::Duration> {
        None
    }
    fn volume(&self) -> f32 {
        1.0
    }
    fn repeat_mode(&self) -> RepeatMode {
        RepeatMode::Off
    }
    fn shuffle(&self) -> bool {
        false
    }
    fn shuffle_scope(&self) -> Option<&str> {
        None
    }
    fn status_message(&self) -> Option<&str> {
        None
    }
    fn queue(&self) -> &[QueueEntry] {
        &[]
    }
    fn module_info(&self) -> Option<&ModuleInfo> {
        None
    }
    fn fft(&self) -> Vec<f32> {
        Vec::new()
    }
    fn samples(&self) -> Vec<f32> {
        Vec::new()
    }
    fn play_pause(&mut self) {}
    fn stop(&mut self) {}
    fn next(&mut self) {}
    fn previous(&mut self) {}
    fn seek(&mut self, _position: std::time::Duration) {}
    fn set_volume(&mut self, _volume: f32) {}
    fn set_repeat_mode(&mut self, _mode: RepeatMode) {}
    fn set_shuffle(&mut self, _enabled: bool) {}
    fn queue_jump(&mut self, _index: usize) {}
    fn queue_remove(&mut self, _index: usize) {}
    fn replace_and_play(&mut self, _paths: &[PathBuf], _start_index: usize) {}
    fn play_shuffled(&mut self, _paths: &[PathBuf], _label: &str) {}
    fn play_next(&mut self, _path: &Path) {}
    fn enqueue(&mut self, _path: &Path) {}
}
