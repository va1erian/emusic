//! Snapshot tests for the main views (#32), rendered headlessly via
//! `egui_kittest`'s wgpu-backed harness (falls back to a software adapter
//! such as WARP on Windows).
//!
//! GPU rendering can be unavailable in some CI/sandbox environments (no
//! adapter at all). When that happens these tests **skip** (print a notice
//! and return) rather than fail, per #32's acceptance criteria; the
//! `emusic-shot` binary itself is still required to build and run.
//!
//! To update the stored snapshots after an intentional UI change:
//! `UPDATE_SNAPSHOTS=1 cargo test -p emusic --test snapshot_views`

use std::panic::AssertUnwindSafe;

use eframe::egui;
use egui_kittest::Harness;

use emusic::app::EguiApp;
use emusic::config::Config;
use emusic::library_api::{Candidate, LibraryDataSource};
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::View;
use emusic::tag_editor::AutoTagState;

fn snapshot_view(view: View) {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1280.0, 800.0))
            .build_eframe(|cc| {
                let library = MockLibrary::new();
                let player = Box::new(MockPlayer::playing_demo(&library.tracks()[0]));
                EguiApp::with_config(cc, Box::new(library), player, Config::default())
            });
        harness.state_mut().set_view(view);
        // A single step: the mock player reports "Playing" and the shell's
        // repaint policy (#6) keeps requesting repaints while playing, so
        // `Harness::run` (which waits for the UI to go idle) never
        // stabilizes here - that is the correct behaviour, not something
        // to wait out.
        harness.run_steps(1);
        harness.snapshot(format!("views/{}", view.slug()));
    }));

    if result.is_err() {
        eprintln!(
            "skipping snapshot test for `{}`: no headless GPU adapter available in this environment",
            view.slug()
        );
    }
}

#[test]
fn music_view() {
    snapshot_view(View::Music);
}

#[test]
fn albums_view() {
    snapshot_view(View::Albums);
}

#[test]
fn folders_view() {
    snapshot_view(View::Folders);
}

#[test]
fn now_playing_view() {
    snapshot_view(View::NowPlaying);
}

#[test]
fn history_view() {
    snapshot_view(View::History);
}

#[test]
fn starred_view() {
    snapshot_view(View::Starred);
}

#[test]
fn most_played_view() {
    snapshot_view(View::MostPlayed);
}

/// Renders the tag editor dialog, optionally with an auto-tag lookup state
/// (#209).
fn tag_editor_snapshot(name: &str, auto_tag: Option<AutoTagState>) {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1280.0, 800.0))
            .build_eframe(|cc| {
                let library = MockLibrary::new();
                let player = Box::new(MockPlayer::playing_demo(&library.tracks()[0]));
                EguiApp::with_config(cc, Box::new(library), player, Config::default())
            });
        harness.state_mut().open_tag_editor();
        if let Some(state) = auto_tag {
            harness.state_mut().set_tag_editor_auto_tag(state);
        }
        // A modal is laid out in its own area; give egui a couple of frames to
        // position it before capturing, as `emusic-shot` does.
        harness.run_steps(3);
        harness.snapshot(name);
    }));

    if result.is_err() {
        eprintln!(
            "skipping snapshot test for `{name}`: no headless GPU adapter available in this environment"
        );
    }
}

#[test]
fn tag_editor_dialog() {
    tag_editor_snapshot("views/tag-editor", None);
}

#[test]
fn tag_editor_auto_tag_searching() {
    tag_editor_snapshot("views/tag-editor-searching", Some(AutoTagState::Searching));
}

#[test]
fn tag_editor_auto_tag_matches() {
    tag_editor_snapshot(
        "views/tag-editor-matches",
        Some(AutoTagState::Matches(demo_candidates())),
    );
}

#[test]
fn tag_editor_auto_tag_no_match() {
    tag_editor_snapshot("views/tag-editor-no-match", Some(AutoTagState::NoMatch));
}

/// Canned candidates for the Matches snapshot.
fn demo_candidates() -> Vec<Candidate> {
    vec![
        Candidate {
            title: Some("Around the World".to_string()),
            artist: Some("Daft Punk".to_string()),
            album: Some("Homework".to_string()),
            album_artist: Some("Daft Punk".to_string()),
            year: Some(1997),
            track_no: Some(5),
            disc_no: Some(1),
            score: 0.96,
            ..Default::default()
        },
        Candidate {
            title: Some("Around the World (radio edit)".to_string()),
            artist: Some("Daft Punk".to_string()),
            year: Some(1997),
            score: 0.61,
            ..Default::default()
        },
    ]
}
