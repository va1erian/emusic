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

use emusic::app::App;
use emusic::config::Config;
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::View;

#[test]
fn dragging_the_tree_panel_edge_widens_it() {
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 800.0))
        .build_eframe(|cc| {
            let library = MockLibrary::new();
            let player = Box::new(MockPlayer::default());
            App::with_config(cc, Box::new(library), player, Config::default())
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
