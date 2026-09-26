//! Headless smoke test for the app (#106): builds the real
//! [`Win32App`] against the mock backends, lets it tick, and quits. Skips
//! (prints and returns) if the session cannot create windows.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;

use emusic::app::{Msg, Win32App};
use emusic_ui::config::Config;
use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::mock::{MockLibrary, MockPlayer};
use emusic_ui::waker::WakerSlot;
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
        WindowSpec::new("emusic.smoke").theme(Theme::light()),
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
        WindowSpec::new("emusic.smoke.playing").theme(Theme::light()),
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
        WindowSpec::new("emusic.settings").theme(Theme::dark()),
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
        WindowSpec::new("emusic.artists").theme(Theme::light()),
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
        WindowSpec::new("emusic.genres").theme(Theme::dark()),
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

/// Exercises the Most Played view wiring (#245): navigating there must build
/// the window selector and the ranked track table from the mock library and
/// tick without panicking.
#[test]
fn most_played_view_builds_its_table_and_quits() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.most-played").theme(Theme::dark()),
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
            ui.emit(Msg::Navigate(emusic_ui::state::View::MostPlayed));
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

/// Exercises the History view wiring (#246): navigating there must build the
/// day-grouped play list from the mock library and tick without panicking.
#[test]
fn history_view_builds_its_model_and_quits() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.history").theme(Theme::dark()),
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
            ui.emit(Msg::Navigate(emusic_ui::state::View::History));
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

/// Exercises the preset browser wiring (#338): navigating to the
/// Visualization view, filtering, selecting and playing a row must rebuild the
/// list and dispatch the play commands without panicking.
#[test]
fn preset_browser_view_filters_selects_and_plays() {
    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.preset-browser").theme(Theme::dark()),
        move |ui| {
            let mut app = Win32App::new(
                ui,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                Config::default(),
                None,
                None,
                None,
                WakerSlot::new(),
            );
            // Seed the deterministic placeholder list a `--mock`/shot run uses.
            app.seed_placeholder_presets();
            ui.emit(Msg::Navigate(emusic_ui::state::View::Visualization));
            ui.emit(Msg::PresetFilter("dancer".to_owned()));
            ui.emit(Msg::PresetSelect(0));
            ui.emit(Msg::PresetPlay(0));
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
        WindowSpec::new("emusic.albums").theme(Theme::dark()),
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

/// Exercises the independent visualization window (#303): with the layout
/// pointing at the window surface, the app must open it, hide it on
/// `SetVisible(false)` (without quitting), and re-show it after `SetDock`
/// without panicking. Skips if the session cannot create windows.
#[test]
fn visualization_window_opens_hides_and_reopens() {
    use emusic_ui::state::VizCommand;
    use emusic_ui::state::projectm::{VizDock, VizLayout};

    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.viz-window").theme(Theme::dark()),
        move |ui| {
            let config = Config {
                projectm_layout: VizLayout {
                    visible: true,
                    dock: VizDock::Window,
                    ..VizLayout::default()
                },
                ..Config::default()
            };
            let app = Win32App::new(
                ui,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                config,
                None,
                None,
                None,
                WakerSlot::new(),
            );
            // Closing the window hides the visualization, it does not quit.
            ui.emit(Msg::Viz(VizCommand::SetVisible(false)));
            ui.emit(Msg::Timer);
            // Docking back to the window shows the same (still open) window.
            ui.emit(Msg::Viz(VizCommand::SetDock(VizDock::Window)));
            ui.emit(Msg::Timer);
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

/// Exercises live appearance changes (#309): building with a non-default saved
/// font size, density and zebra flag, then changing all three through the
/// Appearance page messages, must relayout every list without panicking.
#[test]
fn appearance_changes_apply_live_and_quit() {
    use emusic::views::settings::SettingsMsg;
    use emusic_ui::state::{Appearance, Density, FontSize};

    let constructed = Rc::new(Cell::new(false));
    let constructed_for_make = Rc::clone(&constructed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.appearance").theme(Theme::dark()),
        move |ui| {
            let config = Config {
                appearance: Appearance {
                    font_size: FontSize::Large,
                    density: Density::Spacious,
                    zebra: false,
                },
                ..Config::default()
            };
            let app = Win32App::new(
                ui,
                Box::new(MockLibrary::new()),
                Box::new(MockPlayer::default()),
                config,
                None,
                None,
                None,
                WakerSlot::new(),
            );
            ui.emit(Msg::Navigate(emusic_ui::state::View::Settings));
            // Each of these ticks the app, applying the new metrics live.
            ui.emit(Msg::Settings(SettingsMsg::SetFontSize(FontSize::Small)));
            ui.emit(Msg::Settings(SettingsMsg::SetDensity(Density::Compact)));
            ui.emit(Msg::Settings(SettingsMsg::ToggleZebra(true)));
            ui.emit(Msg::Settings(SettingsMsg::SetFontSize(FontSize::Larger)));
            ui.emit(Msg::Settings(SettingsMsg::SetDensity(Density::Spacious)));
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
