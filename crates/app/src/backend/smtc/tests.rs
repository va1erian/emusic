//! Unit tests for SMTC metadata mapping, the event -> command mapping, the
//! disabled (no-op) path and the off-thread cover lookup.

use std::path::PathBuf;
use std::time::Instant;

use emusic_ui::mock::{MockLibrary, MockPlayer};

use super::*;

fn player_info(title: &str, artist: &str, album: &str) -> NowPlayingInfo {
    NowPlayingInfo {
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        path: "C:\\music\\song.mp3".to_string(),
        duration: Duration::from_secs(180),
    }
}

#[test]
fn library_tags_win_over_player_labels() {
    let player = player_info("song", "", "");
    let track = TrackInfo {
        title: "Real Title".to_string(),
        artist: "Real Artist".to_string(),
        album: "Real Album".to_string(),
        duration: Duration::from_secs(200),
        ..TrackInfo::default()
    };
    let meta = metadata(Some(&player), Some(&track));
    assert_eq!(meta.title.as_deref(), Some("Real Title"));
    assert_eq!(meta.artist.as_deref(), Some("Real Artist"));
    assert_eq!(meta.album.as_deref(), Some("Real Album"));
    assert_eq!(meta.duration, Some(Duration::from_secs(180)));
}

#[test]
fn missing_tags_fall_back_to_player_labels() {
    let player = player_info("song", "unknown", "");
    let meta = metadata(Some(&player), None);
    assert_eq!(meta.title.as_deref(), Some("song"));
    assert_eq!(meta.artist.as_deref(), Some("unknown"));
    assert_eq!(meta.album, None);
}

#[test]
fn nothing_playing_blanks_the_title() {
    let meta = metadata(None, None);
    assert_eq!(meta.title.as_deref(), Some(""));
    assert_eq!(meta.artist, None);
}

#[test]
fn transport_events_map_to_the_shared_commands() {
    use MediaControlEvent as Event;

    let playing = PlaybackStatus::Playing;
    let paused = PlaybackStatus::Paused;
    let stopped = PlaybackStatus::Stopped;

    // Toggle always toggles; Play/Pause are gated on the current status so a
    // stale overlay button never does the opposite of what it says.
    assert_eq!(
        transport_command(Event::Toggle, playing, true),
        Some(Command::PlayerPlayPause)
    );
    assert_eq!(
        transport_command(Event::Play, paused, true),
        Some(Command::PlayerPlayPause)
    );
    assert_eq!(
        transport_command(Event::Play, stopped, true),
        Some(Command::PlayerPlayPause)
    );
    assert_eq!(transport_command(Event::Play, playing, true), None);
    assert_eq!(
        transport_command(Event::Pause, playing, true),
        Some(Command::PlayerPlayPause)
    );
    assert_eq!(transport_command(Event::Pause, paused, true), None);

    assert_eq!(
        transport_command(Event::Next, playing, true),
        Some(Command::PlayerNext)
    );
    assert_eq!(
        transport_command(Event::Previous, playing, true),
        Some(Command::PlayerPrevious)
    );
    assert_eq!(
        transport_command(Event::Stop, playing, true),
        Some(Command::PlayerStop)
    );

    // The scrubber seek is dropped when the current channel ignores seeks.
    let seek = Event::SetPosition(MediaPosition(Duration::from_secs(5)));
    assert_eq!(
        transport_command(seek, playing, true),
        Some(Command::PlayerSeek(Duration::from_secs(5)))
    );
    assert_eq!(
        transport_command(
            Event::SetPosition(MediaPosition(Duration::from_secs(5))),
            playing,
            false
        ),
        None
    );
}

#[test]
fn smtc_without_a_window_is_a_no_op() {
    // On Windows souvlaki would panic without an HWND; the guard must return a
    // disabled integration instead, and `sync` must not touch the player.
    let mut smtc = Smtc::new(None);
    assert!(smtc.controls.is_none(), "disabled when there is no window");
    assert!(smtc.events.is_none());

    let player = MockPlayer::default();
    let library = MockLibrary::new();
    let mut state = AppState::default();
    smtc.sync(&player, &library, &mut state);
    assert!(state.pending.is_empty(), "the disabled path queues nothing");
}

/// A unique, empty scratch directory for one test.
fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emusic-smtc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Drains `lookup` until `path` resolves or the timeout elapses. The outer
/// `None` means the timeout expired; the inner value is the cover URL.
fn wait_for(lookup: &mut CoverLookup, path: &str) -> Option<Option<String>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        lookup.drain();
        if lookup.cache.contains_key(path) {
            return Some(lookup.url(path));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn cover_lookup_resolves_a_sibling_image() {
    let dir = scratch_dir("sibling");
    let track = dir.join("song.mp3");
    std::fs::write(&track, b"").expect("write track");
    std::fs::write(dir.join("cover.jpg"), b"").expect("write cover");

    let path = track.to_str().expect("utf-8 path");
    let mut lookup = CoverLookup::default();
    lookup.request(path);
    let url = wait_for(&mut lookup, path)
        .expect("lookup finished before the timeout")
        .expect("sibling cover was found");
    assert!(url.starts_with("file://"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cover_lookup_caches_a_missing_cover() {
    let dir = scratch_dir("missing");
    let track = dir.join("song.mp3");
    std::fs::write(&track, b"").expect("write track");

    let path = track.to_str().expect("utf-8 path");
    let mut lookup = CoverLookup::default();
    lookup.request(path);
    let url = wait_for(&mut lookup, path).expect("lookup finished before the timeout");
    assert!(url.is_none());

    // A second request must reuse the cached result instead of spawning again.
    lookup.request(path);
    assert!(lookup.loading.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
