//! Regression test for #137: with a long library-folder list, the Settings
//! → Library folder list must scroll within its own bounded area instead of
//! growing the page without bound (which used to push the rest of Settings
//! — and the lower folders themselves — out of reach).
//!
//! Uses `egui_kittest`'s accesskit tree only (no GPU render): a folder that
//! starts well below the visible area becomes queryable after a scroll wheel
//! event over the list, confirming the list scrolled rather than overflowed.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use emusic::app::App;
use emusic::config::Config;
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::{SettingsTab, View};

/// Enough folders that only the first handful fit in a 1280x800 window.
const FOLDER_COUNT: usize = 60;

/// A well-below-the-fold folder, invisible until the list scrolls.
const HIDDEN_FOLDER: &str = "D:/Music/Library/album-050";

fn library_harness() -> Harness<'static, App> {
    let config = Config {
        library_folders: (0..FOLDER_COUNT)
            .map(|i| format!("D:/Music/Library/album-{i:03}").into())
            .collect(),
        ..Config::default()
    };

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 800.0))
        .build_eframe(move |cc| {
            let library = MockLibrary::empty();
            let player = Box::new(MockPlayer::default());
            App::with_config(cc, Box::new(library), player, config)
        });
    harness.state_mut().set_view(View::Settings);
    harness.state_mut().set_settings_tab(SettingsTab::Library);
    harness.run_steps(2);
    harness
}

#[test]
fn long_folder_list_scrolls_within_the_page() {
    let mut harness = library_harness();

    let before = harness
        .query_by_label(HIDDEN_FOLDER)
        .expect("album-050 is present in the list")
        .rect();
    assert!(
        before.min.y > 800.0,
        "album-050 should start below the 800px-tall page (was {before:?})"
    );

    // Point at the middle of the list and scroll down; the inner scroll area
    // must consume the wheel event and bring the lower folders up.
    harness.event(egui::Event::PointerMoved(egui::pos2(300.0, 400.0)));
    for _ in 0..20 {
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, -3.0),
            modifiers: egui::Modifiers::default(),
            phase: egui::TouchPhase::Move,
        });
    }
    harness.run_steps(3);

    let after = harness
        .query_by_label(HIDDEN_FOLDER)
        .expect("album-050 is still present after scrolling")
        .rect();
    assert!(
        after.min.y < 800.0,
        "scrolling the folder list should bring album-050 into view (was {after:?})"
    );
}
