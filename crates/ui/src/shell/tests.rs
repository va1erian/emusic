//! Shell unit tests: drive commands against the mock backends (no GUI), per
//! #97.

use super::*;
use crate::mock::{MockLibrary, MockPlayer};

fn shell() -> Shell {
    Shell::new(
        Box::new(MockLibrary::new()),
        Box::new(MockPlayer::default()),
        Config::default(),
        None,
        WakerSlot::new(),
    )
}

#[test]
fn dispatch_play_track_starts_playback_on_the_next_tick() {
    let mut shell = shell();
    let id = shell.library.tracks()[0].id;

    shell.dispatch(Command::PlayTrack {
        id,
        context: vec![],
    });
    assert_eq!(shell.player.status(), PlaybackStatus::Stopped);

    shell.tick(Instant::now());

    assert_eq!(shell.player.status(), PlaybackStatus::Playing);
    assert_eq!(shell.state.pending.len(), 0, "commands are drained");
}

#[test]
fn tick_reports_a_now_playing_change() {
    let mut shell = shell();
    let id = shell.library.tracks()[0].id;

    shell.dispatch(Command::PlayTrack {
        id,
        context: vec![],
    });
    let tick = shell.tick(Instant::now());

    assert!(tick.changes.contains(Changes::NOW_PLAYING));
}

#[test]
fn tick_wakes_while_playing_and_is_idle_when_stopped() {
    let mut shell = shell();
    assert_eq!(shell.tick(Instant::now()).next_wake, None);

    let id = shell.library.tracks()[0].id;
    shell.dispatch(Command::PlayTrack {
        id,
        context: vec![],
    });
    assert_eq!(
        shell.tick(Instant::now()).next_wake,
        Some(Duration::from_secs(1))
    );
}

#[test]
fn visualizer_narrows_the_wake_interval() {
    let mut shell = shell();
    shell.state.visualizer_enabled = true;
    shell.state.visualizer = VisualizerMode::Spectrum;
    let id = shell.library.tracks()[0].id;
    shell.dispatch(Command::PlayTrack {
        id,
        context: vec![],
    });

    assert_eq!(shell.tick(Instant::now()).next_wake, Some(FRAME_INTERVAL));
}

#[test]
fn dispatch_toggle_theme_changes_theme() {
    let mut shell = shell();
    shell.dispatch(Command::ToggleTheme);
    let tick = shell.tick(Instant::now());

    assert_eq!(shell.state.theme, Theme::Light);
    assert!(tick.changes.contains(Changes::THEME));
}

#[test]
fn dispatch_toggle_panel_changes_visibility() {
    let mut shell = shell();
    let before = shell.state.panels.navigator;
    shell.dispatch(Command::TogglePanel(crate::state::PanelKind::Navigator));
    let tick = shell.tick(Instant::now());

    assert_eq!(shell.state.panels.navigator, !before);
    assert!(tick.changes.contains(Changes::PANELS));
}

#[test]
fn toggle_starred_flips_the_track_flag() {
    let mut shell = shell();
    let id = shell.library.tracks()[0].id;
    let before = shell.library.tracks()[0].starred;

    shell.dispatch(Command::ToggleStarred(id));
    shell.tick(Instant::now());

    let after = shell
        .library
        .tracks()
        .iter()
        .find(|track| track.id == id)
        .expect("track still exists")
        .starred;
    assert_eq!(after, !before);
}

#[test]
fn with_config_disables_persistence() {
    let mut shell = shell();
    assert!(shell.config_path.is_none());
    shell.state.theme = Theme::Light;
    // Would panic or write if persistence were enabled with a bogus path.
    shell.tick(Instant::now());
}
