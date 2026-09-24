//! Regression test: the Folders view's directory tree panel must stay
//! resizable by dragging its edge.
//!
//! The tree panel used to be an `egui::Panel::left` nested inside the
//! central view's `ScrollArea`, which computed its docking rect from the
//! scroll content's bounds instead of the screen — breaking both its
//! layering (it could paint over the navigator) and its resize-drag hit
//! test. It's now a real top-level panel (see `views::folders::tree_panel`),
//! so dragging its right edge should widen it and push the track table over,
//! same as the navigator/right panel.
//!
//! Uses `egui_kittest`'s accesskit tree only (no GPU render, per #32's
//! precedent in `snapshot_views.rs`).

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use emusic::app::EguiApp;
use emusic::config::Config;
use emusic::library_api::{
    AlbumInfo, ArtistInfo, DirNodeInfo, FolderInfo, GenreInfo, HistoryEntry, LibraryDataSource,
    StatsWindow, TrackInfo,
};
use emusic::mock::MockPlayer;
use emusic::state::View;

/// A single-node library whose one folder has a name much longer than any
/// mock name (#18's real-library complaint: `Config::default()` names are
/// short, so tests using [`emusic::mock::MockLibrary`] never exercised a row
/// wide enough to trigger this).
struct LongNameLibrary {
    dirs: Vec<DirNodeInfo>,
}

impl LongNameLibrary {
    fn new() -> Self {
        Self {
            dirs: vec![DirNodeInfo {
                path: "Z:/Music/1979 A Single Man in Moscow".into(),
                name: "1979 A Single Man in Moscow (Bootleg Moscow 1979-05-28 Radio Broadcast, 2 CD) @320".into(),
                direct_track_count: 26,
                total_track_count: 26,
                children: Vec::new(),
            }],
        }
    }
}

impl LibraryDataSource for LongNameLibrary {
    fn tracks(&self) -> &[TrackInfo] {
        &[]
    }
    fn albums(&self) -> &[AlbumInfo] {
        &[]
    }
    fn artists(&self) -> &[ArtistInfo] {
        &[]
    }
    fn genres(&self) -> &[GenreInfo] {
        &[]
    }
    fn folders(&self) -> &[FolderInfo] {
        &[]
    }
    fn dir_tree(&self) -> &[DirNodeInfo] {
        &self.dirs
    }
    fn history(&self) -> &[HistoryEntry] {
        &[]
    }
    fn most_played(&self, _window: StatsWindow) -> &[TrackInfo] {
        &[]
    }
}

#[test]
fn dragging_the_tree_panel_edge_widens_it() {
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 800.0))
        .build_eframe(|cc| {
            let library = emusic::mock::MockLibrary::new();
            let player = Box::new(MockPlayer::default());
            EguiApp::with_config(cc, Box::new(library), player, Config::default())
        });
    harness.state_mut().set_view(View::Folders);
    harness.run_steps(2);

    let heading_before = harness
        .query_by_label("All folders")
        .expect("the Folders view shows an 'All folders' heading by default")
        .rect();

    // The tree panel's right (resizable) edge sits just left of the central
    // view's own 8px inner margin, which is where `heading_before` starts.
    // The grab radius is only 3px (`Style::interaction.resize_grab_radius_side`),
    // so the drag must start right on it. Drag it 80px further right.
    let edge_x = heading_before.min.x - 8.0;
    let y = 400.0;
    harness.drag_at(egui::pos2(edge_x, y));
    harness.run_steps(1);
    harness.hover_at(egui::pos2(edge_x + 40.0, y));
    harness.run_steps(1);
    harness.hover_at(egui::pos2(edge_x + 80.0, y));
    harness.run_steps(1);
    harness.drop_at(egui::pos2(edge_x + 80.0, y));
    harness.run_steps(3);

    let heading_after = harness
        .query_by_label("All folders")
        .expect("the heading is still there after resizing")
        .rect();

    assert!(
        heading_after.min.x > heading_before.min.x + 40.0,
        "dragging the tree panel's edge right should widen it and push the \
         central view over (before {heading_before:?}, after {heading_after:?})"
    );
}

/// The tree panel's max width used to be a fixed 460px, tuned around the
/// mock library's short names — too narrow for a real library's longer
/// paths. It's now sized dynamically off the window width (minus room for
/// the track table), so a wide window should let it grow well past 460px.
#[test]
fn the_tree_panel_can_grow_past_the_old_fixed_cap() {
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1600.0, 900.0))
        .build_eframe(|cc| {
            let library = emusic::mock::MockLibrary::new();
            let player = Box::new(MockPlayer::default());
            EguiApp::with_config(cc, Box::new(library), player, Config::default())
        });
    harness.state_mut().set_view(View::Folders);
    harness.run_steps(2);

    let heading_before = harness
        .query_by_label("All folders")
        .expect("the Folders view shows an 'All folders' heading by default")
        .rect();
    let edge_x = heading_before.min.x - 8.0;
    let y = 400.0;

    // Drag 500px right: 260 (default) + 500 = 760, well past the old 460
    // fixed cap.
    harness.drag_at(egui::pos2(edge_x, y));
    harness.run_steps(1);
    for step in 1..=5 {
        harness.hover_at(egui::pos2(edge_x + step as f32 * 100.0, y));
        harness.run_steps(1);
    }
    harness.drop_at(egui::pos2(edge_x + 500.0, y));
    harness.run_steps(3);

    let heading_after = harness
        .query_by_label("All folders")
        .expect("the heading is still there after resizing")
        .rect();

    assert!(
        heading_after.min.x > heading_before.min.x + 300.0,
        "the tree panel should be able to grow well past the old 460px cap \
         on a wide window (before {heading_before:?}, after {heading_after:?})"
    );
}

/// A long real-library folder name (unlike the mock library's short ones)
/// used to silently override a narrower width the user dragged the panel
/// down to: `ui.selectable_label`'s un-truncated `Button` requested its full
/// natural width in the row's plain (non-wrapping) `ui.horizontal`, and that
/// overflow fed back into `egui::Panel`'s own size measurement (its outer
/// rect is the *rendered* content rect, only clamped against the range's
/// *max*), pinning the panel at whatever its widest row needed — so once
/// widened to fit a long name, it could never shrink back down.
#[test]
fn a_long_folder_name_does_not_pin_the_panel_open() {
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 800.0))
        .build_eframe(|cc| {
            let library = LongNameLibrary::new();
            let player = Box::new(MockPlayer::default());
            EguiApp::with_config(cc, Box::new(library), player, Config::default())
        });
    harness.state_mut().set_view(View::Folders);
    harness.run_steps(2);

    let heading = harness
        .query_by_label("All folders")
        .expect("the Folders view shows an 'All folders' heading by default")
        .rect();
    let edge_x = heading.min.x - 8.0;
    let y = 400.0;

    // Drag the panel down toward its 180px minimum — well narrower than the
    // long folder name's natural rendered width.
    harness.drag_at(egui::pos2(edge_x, y));
    harness.run_steps(1);
    harness.hover_at(egui::pos2(edge_x - 200.0, y));
    harness.run_steps(1);
    harness.drop_at(egui::pos2(edge_x - 200.0, y));
    harness.run_steps(3);

    let heading_after = harness
        .query_by_label("All folders")
        .expect("the heading is still there after resizing")
        .rect();

    assert!(
        heading_after.min.x < heading.min.x - 40.0,
        "a long folder name shouldn't stop the tree panel from shrinking \
         (before {heading:?}, after {heading_after:?})"
    );
}
