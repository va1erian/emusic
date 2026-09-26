//! Keyboard-shortcut integration checks (#28): the real [`Win32App`] against
//! the mock backends, exercising the accelerator table and the
//! `Msg::Shortcut` dispatch path end to end. Skips (prints and returns) if the
//! session cannot create windows, mirroring `tests/smoke.rs`.
//!
//! The app owns its player behind a `Box<dyn PlayerApi>`, so the tests wrap the
//! mock player in a recording adapter that keeps a shared log of the calls the
//! shell made (play/pause, seek, volume, ...). That is the only observable
//! state; it is not a reimplementation of the app.

#![cfg(windows)]

use std::cell::{Cell, RefCell};
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
use win32ui::prelude::*;

/// `WM_KEYDOWN` (`winuser.h`).
const WM_KEYDOWN: u32 = 0x0100;
/// `VK_SPACE` (`winuser.h`).
const VK_SPACE: usize = 0x20;
/// Milliseconds after which the watchdog gives up on the app.
const WATCHDOG_MS: u32 = 8000;
/// Milliseconds the posted-key test waits for the accelerator before quitting.
const KEY_SETTLE_MS: u32 = 500;

/// What the shell asked the player to do, so the test can assert on it after
/// the app has taken ownership.
#[derive(Clone, Default)]
struct Calls {
    play_pause: u32,
    stop: u32,
    next: u32,
    previous: u32,
    seeks: Vec<Duration>,
    volumes: Vec<f32>,
    /// The player status after each `play_pause`/`stop`.
    statuses: Vec<PlaybackStatus>,
}

/// A [`MockPlayer`] that records the shell's transport calls in a shared log.
struct RecordingPlayer {
    inner: MockPlayer,
    calls: Rc<RefCell<Calls>>,
}

impl RecordingPlayer {
    /// Wraps `inner`, returning the shared log for the test to read.
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

// The getters delegate straight to the inner player, so references into it stay
// valid; only the mutators touch the log.
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
        let mut calls = self.calls.borrow_mut();
        calls.stop += 1;
        calls.statuses.push(self.inner.status());
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
        None,
        WakerSlot::new(),
    );
    (app, calls)
}

/// Installs a watchdog so a stuck app fails instead of hanging the test.
/// Call it *after* [`Win32App::new`], which installs the app's own timer
/// handler.
fn watchdog(ui: &Ui<Msg>) {
    let watchdog = ui.set_timer(WATCHDOG_MS).ok();
    ui.on_timer(move |fired| {
        if Some(fired) == watchdog {
            win32ui::quit(1);
        }
        None
    });
}

/// A handler that ignores every message; only needed to own the injector
/// window.
struct NullHandler;

impl WindowHandler for NullHandler {
    fn message(&self, _window: &Window, _message: Message) -> Option<LResult> {
        None
    }
}

/// Posts `WM_KEYDOWN` for `vk` into the app's message loop, the way a real key
/// arrives. `win32ui` exposes no way to post to an arbitrary `Hwnd`, so this
/// makes a tiny child of the app window and posts to that: the loop resolves
/// the app window as its root ancestor and offers the key to the app's
/// accelerator table. The window is leaked so it lives as long as the loop.
///
/// Returns `false` when the window cannot be created, so the test can skip.
fn inject_key(parent: Hwnd, theme: Theme, vk: usize) -> bool {
    let Ok(class) = WindowClass::register("emusic.shortcuts.inject", theme.background) else {
        return false;
    };
    let Ok(window) = Window::create(
        class,
        Some(parent),
        WindowStyle::new().child(),
        WindowExStyle::new(),
        Rect::new(0, 0, 1, 1),
        "emusic.shortcuts.inject",
        NullHandler,
    ) else {
        return false;
    };
    let _ = window.post_message(WM_KEYDOWN, vk, 0);
    std::mem::forget(window);
    true
}

/// The accelerator table installs and the app ticks and quits without
/// panicking.
#[test]
fn accelerator_table_installs_and_app_ticks() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.shortcuts.install").theme(Theme::dark()),
        move |ui| {
            let (app, _calls) = build_app(ui, MockPlayer::default());
            constructed_for_make.set(true);
            watchdog(ui);
            ui.emit(Msg::Quit);
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    assert!(constructed.get(), "the app was never constructed");
}

/// A posted `WM_KEYDOWN` for the bare Space binding reaches the accelerator
/// table and toggles playback. Unmodified keys need no live modifier state, so
/// this works with synthetic input; Ctrl combinations cannot be posted this way.
#[test]
fn posted_space_toggles_playback() {
    let calls = Rc::new(RefCell::new(None));
    let injected = Rc::new(Cell::new(false));
    let calls_for_make = Rc::clone(&calls);
    let injected_for_make = Rc::clone(&injected);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.shortcuts.space").theme(Theme::dark()),
        move |ui| {
            let (app, calls) = build_app(ui, MockPlayer::default());
            calls_for_make.replace(Some(calls));
            // Post the key for the loop to translate; `Win32App::new` has
            // installed the accelerator table by now.
            injected_for_make.set(inject_key(ui.hwnd(), Theme::dark(), VK_SPACE));
            let settle = ui.set_timer(KEY_SETTLE_MS).ok();
            let watchdog = ui.set_timer(WATCHDOG_MS).ok();
            ui.on_timer(move |fired| {
                if Some(fired) == watchdog {
                    win32ui::quit(1);
                } else if Some(fired) == settle {
                    win32ui::quit(0);
                }
                None
            });
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    if !injected.get() {
        eprintln!("skipping: the key injector window could not be created");
        return;
    }
    let outer = calls.borrow();
    let calls = outer.as_ref().expect("the app was constructed");
    let calls = calls.borrow();
    assert!(
        calls.play_pause >= 1,
        "the posted Space key did not fire the PlayPause accelerator"
    );
    assert_eq!(
        calls.statuses.last(),
        Some(&PlaybackStatus::Playing),
        "Space should have started playback from Stopped"
    );
}

/// Emitting [`Msg::Shortcut`] runs the whole dispatch path: the ui helper turns
/// the player snapshot into a command and the shell applies it to the player.
#[test]
fn shortcut_messages_dispatch_to_the_player() {
    let baseline = Rc::new(RefCell::new(None));
    let calls = Rc::new(RefCell::new(None));
    let baseline_for_make = Rc::clone(&baseline);
    let calls_for_make = Rc::clone(&calls);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.shortcuts.dispatch").theme(Theme::dark()),
        move |ui| {
            let (app, calls) = build_app(ui, MockPlayer::default());
            watchdog(ui);
            // The construct tick may already have touched the player; keep a
            // baseline so assertions are about the shortcut messages alone.
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
