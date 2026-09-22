#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use clap::Parser;
use eframe::egui;

use emusic::app::App;
use emusic::library_api::LibraryDataSource;
use emusic::mock;
use emusic::player_api::PlayerApi;

/// emusic: a MusicBee-inspired music player.
#[derive(Parser, Debug)]
#[command(name = "emusic")]
struct Cli {
    /// Run against deterministic fake data instead of a real library/player
    /// backend (no BASS, no database). Useful for development and for
    /// `emusic-shot` screenshots.
    #[arg(long)]
    mock: bool,
}

fn main() -> eframe::Result {
    let cli = Cli::parse();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("emusic")
            .with_inner_size([1200.0, 760.0]),
        ..Default::default()
    };

    eframe::run_native(
        "emusic",
        options,
        Box::new(move |cc| {
            let (library, player) = backends(cli.mock);
            Ok(Box::new(App::new(cc, library, player)))
        }),
    )
}

/// Chooses which [`LibraryDataSource`]/[`PlayerApi`] implementations to run
/// against. Real backends (`crates/library`, `crates/player`) don't exist
/// yet, so the non-mock path currently falls back to an empty stub; issue
/// #11 will wire the real ones in here without touching the rest of the
/// shell.
fn backends(mock: bool) -> (Box<dyn LibraryDataSource>, Box<dyn PlayerApi>) {
    if mock {
        let library = mock::MockLibrary::new();
        // A fixed, arbitrary index into the deterministically seeded mock
        // library, just so "now playing" points at a real track (and thus
        // highlights a real row in the track table) instead of made-up
        // metadata that never matches anything.
        let player = mock::MockPlayer::playing_demo(&library.tracks()[0]);
        (Box::new(library), Box::new(player))
    } else {
        (Box::new(stub::EmptyLibrary), Box::new(stub::StoppedPlayer))
    }
}

/// Trivial placeholder backends used until a real player/library crate is
/// wired in (#11).
mod stub {
    use std::time::Duration;

    use emusic::library_api::{
        AlbumInfo, ArtistInfo, FolderInfo, HistoryEntry, LibraryDataSource, TrackInfo,
    };
    use emusic::player_api::{
        ModuleInfo, NowPlayingInfo, PlaybackStatus, PlayerApi, QueueEntry, RepeatMode,
    };

    pub struct EmptyLibrary;

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

    #[derive(Default)]
    pub struct StoppedPlayer;

    impl PlayerApi for StoppedPlayer {
        fn tick(&mut self, _dt: Duration) {}
        fn status(&self) -> PlaybackStatus {
            PlaybackStatus::Stopped
        }
        fn now_playing(&self) -> Option<&NowPlayingInfo> {
            None
        }
        fn position(&self) -> Duration {
            Duration::ZERO
        }
        fn duration(&self) -> Option<Duration> {
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
        fn seek(&mut self, _position: Duration) {}
        fn set_volume(&mut self, _volume: f32) {}
        fn set_repeat_mode(&mut self, _mode: RepeatMode) {}
        fn set_shuffle(&mut self, _enabled: bool) {}

        fn queue_jump(&mut self, _index: usize) {}
        fn queue_remove(&mut self, _index: usize) {}
    }
}
