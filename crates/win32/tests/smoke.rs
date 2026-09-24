//! Headless smoke test for the native frontend (#106): builds the real
//! [`Win32App`] against the mock backends, lets it tick, and quits. Skips
//! (prints and returns) if the session cannot create windows.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;

use emusic_ui::config::Config;
use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::mock::{MockLibrary, MockPlayer};
use emusic_ui::waker::WakerSlot;
use emusic_win32::app::{Msg, Win32App};
use win32ui::prelude::*;

/// Milliseconds after which the watchdog gives up on the app.
const WATCHDOG_MS: u32 = 5000;

#[test]
fn app_constructs_ticks_and_quits() {
    let constructed = Rc::new(Cell::new(false));
    let exited_early = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);
    let exited_for_make = Rc::clone(&exited_early);

    let result = win32ui::run_app(
        WindowSpec::new("emusic-win32.smoke").theme(Theme::light()),
        move |ui| {
            let watchdog = ui.set_timer(WATCHDOG_MS).ok();
            let exited = Rc::clone(&exited_for_make);
            ui.on_timer(move |fired| {
                if Some(fired) == watchdog {
                    exited.set(true);
                    win32ui::quit(1);
                }
                None
            });

            let app = Win32App::new(
                ui,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                Config::default(),
                None,
                None,
                None,
                WakerSlot::new(),
            );
            constructed_for_make.set(true);
            ui.emit(Msg::Quit);
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    assert!(
        !exited_early.get(),
        "the watchdog fired before the app quit"
    );
    assert!(constructed.get(), "the app was never constructed");
}

/// Builds the panel against a playing mock track with a populated queue and
/// module info, so the panel's non-empty sync path (artwork request, metadata,
/// queue model) runs too. Skips if the session cannot create windows.
#[test]
fn app_constructs_with_a_playing_track_and_queue() {
    let result = win32ui::run_app(
        WindowSpec::new("emusic-win32.smoke.playing").theme(Theme::light()),
        move |ui| {
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
                None,
                WakerSlot::new(),
            );
            ui.emit(Msg::Quit);
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
    }
}

/// Exercises the Settings view wiring (#115): navigating there must build the
/// tab strip and every page's controls and tick without panicking.
#[test]
fn settings_view_builds_its_controls_and_quits() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic-win32.settings").theme(Theme::dark()),
        move |ui| {
            let app = Win32App::new(
                ui,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                Config::default(),
                None,
                None,
                None,
                WakerSlot::new(),
            );
            ui.emit(Msg::Navigate(emusic_ui::state::View::Settings));
            ui.emit(Msg::Quit);
            constructed_for_make.set(true);
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    assert!(constructed.get(), "the app was never constructed");
}

/// Exercises the Artists view wiring (#248): navigating there must build the
/// name+counts list from the mock library and tick without panicking.
#[test]
fn artists_view_builds_its_model_and_quits() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic-win32.artists").theme(Theme::light()),
        move |ui| {
            let app = Win32App::new(
                ui,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                Config::default(),
                None,
                None,
                None,
                WakerSlot::new(),
            );
            ui.emit(Msg::Navigate(emusic_ui::state::View::Artists));
            ui.emit(Msg::Quit);
            constructed_for_make.set(true);
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    assert!(constructed.get(), "the app was never constructed");
}

/// Exercises the Genres view wiring (#250): navigating there must build the
/// name+counts list from the mock library and tick without panicking.
#[test]
fn genres_view_builds_its_model_and_quits() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic-win32.genres").theme(Theme::dark()),
        move |ui| {
            let app = Win32App::new(
                ui,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                Config::default(),
                None,
                None,
                None,
                WakerSlot::new(),
            );
            ui.emit(Msg::Navigate(emusic_ui::state::View::Genres));
            ui.emit(Msg::Quit);
            constructed_for_make.set(true);
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    assert!(constructed.get(), "the app was never constructed");
}

/// Exercises the Albums view wiring (#113): navigating there must build the
/// grid model from the mock library and tick without panicking.
#[test]
fn albums_view_builds_its_model_and_quits() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic-win32.albums").theme(Theme::dark()),
        move |ui| {
            let app = Win32App::new(
                ui,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                Config::default(),
                None,
                None,
                None,
                WakerSlot::new(),
            );
            ui.emit(Msg::Navigate(emusic_ui::state::View::Albums));
            ui.emit(Msg::Quit);
            constructed_for_make.set(true);
            app
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    assert!(constructed.get(), "the app was never constructed");
}
