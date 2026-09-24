//! The Music view's no-tracks state must distinguish "still scanning" from
//! "actually empty" (#80): while a first scan runs the heading reads
//! "Building your music library...", and "Your library is empty" only
//! appears once no scan is running.
//!
//! Rendered headlessly via `egui_kittest`'s accesskit tree. No GPU is needed
//! because the tests only query the tree, never render a frame.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use emusic::app::EguiApp;
use emusic::config::Config;
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::View;

const EMPTY_HEADING: &str = "Your library is empty";
const SCANNING_HEADING: &str = "Building your music library...";

fn music_harness(library: MockLibrary) -> Harness<'static, EguiApp> {
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 800.0))
        .build_eframe(move |cc| {
            EguiApp::with_config(
                cc,
                Box::new(library),
                Box::new(MockPlayer::default()),
                Config::default(),
            )
        });
    harness.state_mut().set_view(View::Music);
    harness.run_steps(1);
    harness
}

#[test]
fn scanning_library_shows_building_heading() {
    let harness = music_harness(MockLibrary::scanning());

    assert!(
        harness.query_by_label(SCANNING_HEADING).is_some(),
        "a mid-scan library should show the building heading"
    );
    assert!(
        harness.query_by_label(EMPTY_HEADING).is_none(),
        "a mid-scan library must not claim it is empty"
    );
}

#[test]
fn idle_empty_library_shows_empty_heading() {
    let harness = music_harness(MockLibrary::empty());

    assert!(
        harness.query_by_label(EMPTY_HEADING).is_some(),
        "an idle empty library should show the empty heading"
    );
    assert!(
        harness.query_by_label(SCANNING_HEADING).is_none(),
        "an idle empty library must not claim a scan is running"
    );
}
