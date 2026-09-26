//! Keyboard-shortcut integration checks (#28): the portable [`Win32App`] against
//! the mock backends, exercising the `Msg::Shortcut` dispatch path end to end.
//!
//! The portable `xui_core` runtime has no accelerator table yet (that is part
//! of the #[375/#376] window-chrome work), so the raw-key injection test of the
//! win32ui version is a documented follow-up; the message path below is the
//! behaviour that survives.
//!
//! The app owns its player behind a `Box<dyn PlayerApi>`, so the test wraps the
//! mock player in a recording adapter that keeps a shared log of the calls the
//! shell made (play/pause, seek, volume, ...). That is the only observable
//! state; it is not a reimplementation of the app.

#![cfg(windows)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use emusic::app::{Msg, Win32App};
use emusic_player::QueueSnapshot;
use emusic_player::tracker::TrackerSettings;
use emusic_ui::config::Config;
use emusic_ui::mock::{MockLibrary, MockPlayer};
use emusic_ui::player_api::{
    ModuleInfo, NowPlayingInfo, PlaybackStatus, PlayerApi, QueueEntry, RepeatMode,
};
use emusic_ui::state::ShortcutAction;
use emusic_ui::waker::WakerSlot;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::{Backend, Result as BackendResult};

/// What the shell asked the player to do, so the test can assert on it after
/// the app has taken ownership.
#[derive(Clone, Default)]
struct Calls {
    play_pause: u32,
    next: u32,
    previous: u32,
    seeks: Vec<Duration>,
    volumes: Vec<f32>,
    statuses: Vec<PlaybackStatus>,
}

/// A [`MockPlayer`] that records the shell's transport calls in a shared log.
struct RecordingPlayer {
    inner: MockPlayer,
    calls: Rc<RefCell<Calls>>,
}

impl RecordingPlayer {
    fn new(inner: MockPlayer) -> (Self, Rc<RefCell<Calls>>) {
        let calls = Rc::new(RefCell::new(Calls::default()));
        (
            Self {
                inner,
                calls: Rc::clone(&calls),
            },
            calls,
        )
    }
}

// The getters delegate straight to the inner player; only the mutators log.
impl PlayerApi for RecordingPlayer {
    fn tick(&mut self, dt: Duration) {
        self.inner.tick(dt);
    }

    fn status(&self) -> PlaybackStatus {
        self.inner.status()
    }

    fn now_playing(&self) -> Option<&NowPlayingInfo> {
        self.inner.now_playing()
    }

    fn position(&self) -> Duration {
        self.inner.position()
    }

    fn duration(&self) -> Option<Duration> {
        self.inner.duration()
    }

    fn volume(&self) -> f32 {
        self.inner.volume()
    }

    fn repeat_mode(&self) -> RepeatMode {
        self.inner.repeat_mode()
    }

    fn shuffle(&self) -> bool {
        self.inner.shuffle()
    }

    fn shuffle_scope(&self) -> Option<&str> {
        self.inner.shuffle_scope()
    }

    fn status_message(&self) -> Option<&str> {
        self.inner.status_message()
    }

    fn queue(&self) -> &[QueueEntry] {
        self.inner.queue()
    }

    fn module_info(&self) -> Option<&ModuleInfo> {
        self.inner.module_info()
    }

    fn fft(&self) -> Vec<f32> {
        self.inner.fft()
    }

    fn samples(&self) -> Vec<f32> {
        self.inner.samples()
    }

    fn play_pause(&mut self) {
        self.inner.play_pause();
        let mut calls = self.calls.borrow_mut();
        calls.play_pause += 1;
        calls.statuses.push(self.inner.status());
    }

    fn stop(&mut self) {
        self.inner.stop();
    }

    fn next(&mut self) {
        self.inner.next();
        self.calls.borrow_mut().next += 1;
    }

    fn previous(&mut self) {
        self.inner.previous();
        self.calls.borrow_mut().previous += 1;
    }

    fn seek(&mut self, position: Duration) {
        self.inner.seek(position);
        self.calls.borrow_mut().seeks.push(position);
    }

    fn set_volume(&mut self, volume: f32) {
        self.inner.set_volume(volume);
        self.calls.borrow_mut().volumes.push(volume);
    }

    fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.inner.set_repeat_mode(mode);
    }

    fn set_shuffle(&mut self, enabled: bool) {
        self.inner.set_shuffle(enabled);
    }

    fn seek_supported(&self) -> bool {
        self.inner.seek_supported()
    }

    fn set_songlengths_path(&mut self, path: Option<&Path>) {
        self.inner.set_songlengths_path(path);
    }

    fn set_sid_fallback_length(&mut self, length: Duration) {
        self.inner.set_sid_fallback_length(length);
    }

    fn set_tracker_settings(&mut self, settings: &TrackerSettings) {
        self.inner.set_tracker_settings(settings);
    }

    fn set_midi_soundfont(&mut self, path: Option<&Path>) {
        self.inner.set_midi_soundfont(path);
    }

    fn queue_jump(&mut self, index: usize) {
        self.inner.queue_jump(index);
    }

    fn queue_remove(&mut self, index: usize) {
        self.inner.queue_remove(index);
    }

    fn replace_and_play(&mut self, paths: &[PathBuf], start_index: usize) {
        self.inner.replace_and_play(paths, start_index);
    }

    fn queue_snapshot(&self) -> QueueSnapshot {
        self.inner.queue_snapshot()
    }

    fn restore_queue(&mut self, snapshot: &QueueSnapshot, position: Duration, play: bool) {
        self.inner.restore_queue(snapshot, position, play);
    }

    fn play_shuffled(&mut self, paths: &[PathBuf], label: &str) {
        self.inner.play_shuffled(paths, label);
    }

    fn play_next(&mut self, path: &Path) {
        self.inner.play_next(path);
    }

    fn enqueue(&mut self, path: &Path) {
        self.inner.enqueue(path);
    }
}

/// Builds the app with a recording mock player and the shared log.
fn build_app(ui: &mut Ui<Msg>, player: MockPlayer) -> (Win32App, Rc<RefCell<Calls>>) {
    let (recording, calls) = RecordingPlayer::new(player);
    let app = Win32App::new(
        ui,
        Box::new(MockLibrary::new()),
        Box::new(recording),
        Config::default(),
        None,
        None,
        Vec::new(),
        WakerSlot::new(),
    );
    (app, calls)
}

/// Emitting [`Msg::Shortcut`] runs the whole dispatch path: the shared helper
/// turns the player snapshot into a command and the shell applies it.
#[test]
fn shortcut_messages_dispatch_to_the_player() {
    let baseline = Rc::new(RefCell::new(None));
    let calls = Rc::new(RefCell::new(None));
    let baseline_for_make = Rc::clone(&baseline);
    let calls_for_make = Rc::clone(&calls);

    let backend: Rc<dyn Backend> = Rc::new(xui::xui_win32::Win32Backend::new());
    let result: BackendResult<()> = xui::xui_core::run_app(
        backend,
        emusic::window::window_spec(800.0, 600.0),
        move |ui| {
            let (app, calls) = build_app(ui, MockPlayer::default());
            baseline_for_make.replace(Some(calls.borrow().clone()));
            calls_for_make.replace(Some(Rc::clone(&calls)));
            ui.emit(Msg::Shortcut(ShortcutAction::SeekForward));
            ui.emit(Msg::Shortcut(ShortcutAction::SeekBackward));
            ui.emit(Msg::Shortcut(ShortcutAction::VolumeUp));
            ui.emit(Msg::Shortcut(ShortcutAction::VolumeDown));
            ui.emit(Msg::Shortcut(ShortcutAction::NextTrack));
            ui.emit(Msg::Shortcut(ShortcutAction::PreviousTrack));
            ui.emit(Msg::Shortcut(ShortcutAction::PlayPause));
            ui.emit(Msg::Quit);
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    let baseline_outer = baseline.borrow();
    let baseline = baseline_outer.as_ref().expect("the app was constructed");
    let calls_outer = calls.borrow();
    let calls = calls_outer.as_ref().expect("the app was constructed");
    let calls = calls.borrow();

    let seeks = &calls.seeks[baseline.seeks.len()..];
    assert_eq!(
        seeks,
        [Duration::from_secs(5), Duration::ZERO],
        "SeekForward then SeekBackward (from position 0)"
    );
    let volumes = &calls.volumes[baseline.volumes.len()..];
    assert_eq!(volumes.len(), 2, "VolumeUp then VolumeDown");
    assert!(
        (volumes[0] - 0.85).abs() < 1e-6 && (volumes[1] - 0.8).abs() < 1e-6,
        "VolumeUp then VolumeDown from the mock's 0.8, got {volumes:?}"
    );
    assert_eq!(calls.next, baseline.next + 1);
    assert_eq!(calls.previous, baseline.previous + 1);
    assert_eq!(calls.play_pause, baseline.play_pause + 1);
    assert_eq!(
        calls.statuses.last(),
        Some(&PlaybackStatus::Playing),
        "PlayPause should have started playback"
    );
}
