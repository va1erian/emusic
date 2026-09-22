//! Chooses and wires up the real backends (#11): the BASS-backed
//! [`emusic_player::Player`] via [`player_adapter::PlayerAdapter`], and
//! single-instance/IPC via `winshell` ([`ipc`]).
//!
//! The real library (`crates/library`) isn't wired in here — that's a
//! separate issue's scope. `--mock` continues to use the in-memory fakes
//! from `crate::mock`.

pub mod ipc;
pub mod player_adapter;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::{info, warn};

use crate::library_api::{
    AlbumInfo, ArtistInfo, FolderInfo, HistoryEntry, LibraryDataSource, TrackInfo,
};
use crate::mock;
use crate::player_api::{
    ModuleInfo, NowPlayingInfo, PlaybackStatus, PlayerApi, QueueEntry, RepeatMode,
};
use player_adapter::PlayerAdapter;

/// The library/player pair the app runs with, plus a startup notice (e.g.
/// "no audio device") the shell should surface instead of silently limping
/// along.
pub struct Backends {
    pub library: Box<dyn LibraryDataSource>,
    pub player: Box<dyn PlayerApi>,
    pub notice: Option<String>,
}

/// Builds the backends for this run: fakes for `--mock`, otherwise a real
/// [`PlayerAdapter`] over BASS. If BASS can't be loaded (missing DLLs, no
/// output device, ...) the app still starts, with an inert player and a
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

    match init_bass_player() {
        Ok(player) => {
            info!("BASS initialized");
            Backends {
                library: Box::new(EmptyLibrary),
                player: Box::new(player),
                notice: None,
            }
        }
        Err(err) => {
            warn!(%err, "audio backend unavailable; starting without playback");
            Backends {
                library: Box::new(EmptyLibrary),
                player: Box::new(UnavailablePlayer),
                notice: Some(format!("Audio unavailable: {err}")),
            }
        }
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

fn init_bass_player() -> anyhow::Result<PlayerAdapter> {
    let bass = bass::Bass::init(-1, 44100).map_err(|err| {
        anyhow::anyhow!(
            "{err} (looked in {}; set EMUSIC_BASS_DIR to override)",
            bass_dir().display()
        )
    })?;
    for result in bass.load_plugins(bass_dir()) {
        if let Err(err) = result.result {
            warn!(plugin = %result.path.display(), %err, "BASS plugin failed to load");
        }
    }
    let backend = Arc::new(emusic_player::BassBackend::new(bass));
    Ok(PlayerAdapter::new(emusic_player::Player::new(backend)))
}

/// Empty library stand-in; wiring the real `crates/library` store into the
/// shell is out of scope for #11 (player + winshell wiring only).
struct EmptyLibrary;

impl LibraryDataSource for EmptyLibrary {
    fn tracks(&self) -> &[TrackInfo] {
        &[]
    }
    fn albums(&self) -> &[AlbumInfo] {
        &[]
    }
    fn artists(&self) -> &[ArtistInfo] {
        &[]
    }
    fn genres(&self) -> &[String] {
        &[]
    }
    fn folders(&self) -> &[FolderInfo] {
        &[]
    }
    fn history(&self) -> &[HistoryEntry] {
        &[]
    }
    fn most_played(&self) -> &[TrackInfo] {
        &[]
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
    fn queue(&self) -> &[QueueEntry] {
        &[]
    }
    fn module_info(&self) -> Option<&ModuleInfo> {
        None
    }
    fn spectrum(&self) -> &[f32] {
        &[]
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
    fn play_next(&mut self, _path: &Path) {}
    fn enqueue(&mut self, _path: &Path) {}
}
