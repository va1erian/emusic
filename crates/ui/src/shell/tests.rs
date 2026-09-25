//! Shell unit tests: drive commands against the mock backends (no GUI), per
//! #97.

use emusic_player::{ExplicitQueueSnapshot, QueueSnapshot};

use super::*;
use crate::mock::{MockLibrary, MockPlayer};
use crate::state::VizCommand;

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
fn tick_reports_an_appearance_change() {
    let mut shell = shell();
    shell.state.appearance.zebra = false;
    let tick = shell.tick(Instant::now());
    assert!(tick.changes.contains(Changes::APPEARANCE));
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

#[test]
fn session_and_ui_state_round_trip_through_the_config_file() {
    let dir = std::env::temp_dir().join(format!("emusic-shell-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let path = dir.join("config.toml");

    let mut first = Shell::new(
        Box::new(MockLibrary::new()),
        Box::new(MockPlayer::default()),
        Config::default(),
        Some(path.clone()),
        WakerSlot::new(),
    );
    first.state.search_query = "ambient".to_owned();
    first.state.window.size = Some([900.0, 650.0]);
    first.player.restore_queue(
        &QueueSnapshot::Explicit(ExplicitQueueSnapshot {
            items: vec![PathBuf::from("a.flac"), PathBuf::from("b.flac")],
            order: vec![0, 1],
            pos: Some(1),
            shuffle: false,
            repeat: emusic_player::RepeatMode::Off,
        }),
        Duration::from_secs(9),
        true,
    );
    first.save_on_exit();

    let saved = config::load(&path);
    let second = Shell::new(
        Box::new(MockLibrary::new()),
        Box::new(MockPlayer::default()),
        saved,
        Some(path.clone()),
        WakerSlot::new(),
    );

    // UI state was restored…
    assert_eq!(second.state.search_query, "ambient");
    assert_eq!(second.state.window.size, Some([900.0, 650.0]));
    // …and so was the queue, paused at the saved position (autoplay is off
    // by default).
    assert_eq!(
        second.player.now_playing().map(|np| np.path.as_str()),
        Some("b.flac")
    );
    assert_eq!(second.player.status(), PlaybackStatus::Paused);
    assert_eq!(second.player.position(), Duration::from_secs(9));

    std::fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn shown_projectm_wakes_at_frame_rate_even_when_stopped() {
    let mut shell = shell();
    assert_eq!(shell.tick(Instant::now()).next_wake, None);

    shell.dispatch(Command::Viz(VizCommand::SetVisible(true)));
    // The frontend reports the surface is actually rendering.
    shell.state.projectm.running = true;
    let tick = shell.tick(Instant::now());
    assert_eq!(tick.next_wake, Some(FRAME_INTERVAL));
    assert!(tick.changes.contains(Changes::VISUALIZATION));

    // While the frontend reports it is not running (minimised or collapsed),
    // the surface is still shown but the shell falls back to its idle cadence.
    shell.state.projectm.running = false;
    assert_eq!(shell.tick(Instant::now()).next_wake, None);

    shell.dispatch(Command::Viz(VizCommand::SetVisible(false)));
    let tick = shell.tick(Instant::now());
    assert_eq!(tick.next_wake, None, "hidden means no frame-rate wakes");
    assert!(tick.changes.contains(Changes::VISUALIZATION));
}

#[test]
fn moving_projectm_reports_a_visualization_change() {
    let mut shell = shell();
    shell.dispatch(Command::Viz(VizCommand::SetVisible(true)));
    shell.tick(Instant::now());

    shell.dispatch(Command::Viz(VizCommand::SetFullscreen(true)));
    assert!(
        shell
            .tick(Instant::now())
            .changes
            .contains(Changes::VISUALIZATION)
    );
    assert!(
        !shell
            .tick(Instant::now())
            .changes
            .contains(Changes::VISUALIZATION),
        "no change, no flag"
    );
}
