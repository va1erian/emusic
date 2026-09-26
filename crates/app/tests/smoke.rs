//! Headless smoke test for the portable app (#106): builds the real
//! [`Win32App`] against the mock backends, lets it tick, and quits. Skips
//! (prints and returns) if the session cannot create windows.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;

use emusic::app::{Msg, Win32App};
use emusic_ui::config::Config;
use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::mock::{MockLibrary, MockPlayer};
use emusic_ui::state::View;
use emusic_ui::waker::WakerSlot;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::{Backend, Result as BackendResult};

/// Runs the portable app on the native backend with `make`.
fn run_ui(make: impl FnOnce(&mut Ui<Msg>) -> Win32App + 'static) -> BackendResult<()> {
    let backend: Rc<dyn Backend> = Rc::new(xui::xui_win32::Win32Backend::new());
    xui::xui_core::run_app(backend, emusic::window::window_spec(1000.0, 700.0), make)
}

/// Constructs the app, emits `msg`, then quits.
fn smoke(extra: Option<Msg>) -> bool {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);
    let result = run_ui(move |ui| {
        let app = Win32App::new(
            ui,
            Box::new(MockLibrary::new()),
            Box::new(MockPlayer::default()),
            Config::default(),
            None,
            None,
            Vec::new(),
            WakerSlot::new(),
        );
        if let Some(msg) = extra {
            ui.emit(msg);
        }
        constructed_for_make.set(true);
        ui.emit(Msg::Quit);
        app
    });
    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return false;
    }
    assert!(constructed.get(), "the app was never constructed");
    true
}

#[test]
fn app_constructs_ticks_and_quits() {
    smoke(None);
}

/// Navigating to each central view builds its view (the Music view's list, or
/// a placeholder) and ticks without panicking.
#[test]
fn every_view_builds_and_quits() {
    for view in View::ALL {
        smoke(Some(Msg::Navigate(view)));
    }
}

/// A playing mock track populates the Music view's model and the shell's
/// now-playing path.
#[test]
fn music_view_builds_with_a_playing_track() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);
    let result = run_ui(move |ui| {
        let library = MockLibrary::new();
        let track = library.tracks().first().cloned().unwrap_or_default();
        let player = MockPlayer::playing_demo(&track);
        let app = Win32App::new(
            ui,
            Box::new(library),
            Box::new(player),
            Config::default(),
            None,
            None,
            Vec::new(),
            WakerSlot::new(),
        );
        constructed_for_make.set(true);
        ui.emit(Msg::Quit);
        app
    });
    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    assert!(constructed.get(), "the app was never constructed");
}

/// The Music view's row activation and sort hooks run the whole dispatch path
/// without panicking.
#[test]
fn music_view_commands_dispatch() {
    let result = run_ui(move |ui| {
        let app = Win32App::new(
            ui,
            Box::new(MockLibrary::new()),
            Box::new(MockPlayer::default()),
            Config::default(),
            None,
            None,
            Vec::new(),
            WakerSlot::new(),
        );
        ui.emit(Msg::PlayRow(0));
        ui.emit(Msg::SortColumn(2));
        ui.emit(Msg::MusicShuffleAll);
        ui.emit(Msg::Quit);
        app
    });
    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
    }
}
