//! Regression test for #135: `--mock` must never touch the real user's
//! `%APPDATA%\emusic\config.toml`. The mock library reports ~80 synthetic
//! folders; if the mock run persisted like a real one, those would overwrite
//! the user's actual `library_folders` (and volume/theme/...).
//!
//! This drives the exact constructor `main.rs` uses for `--mock`
//! ([`App::for_run`] with `mock = true`), snapshots the real config file's
//! bytes before and after a brief run plus shutdown, and asserts they are
//! byte-identical.

use std::fs;
use std::panic::AssertUnwindSafe;

use eframe::egui;
use egui_kittest::Harness;

use emusic::app::App;
use emusic::config;
use emusic::library_api::LibraryDataSource;
use emusic::mock::{MockLibrary, MockPlayer};

#[test]
fn mock_run_does_not_touch_the_real_config() {
    let Some(path) = config::config_path() else {
        eprintln!("skipping: this environment has no config directory");
        return;
    };

    let before = fs::read(&path).ok();

    // No headless GPU adapter is needed to build/step the harness (only to
    // render), but if this environment can't even construct one, skip like
    // the snapshot tests do rather than fail.
    let built = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1280.0, 800.0))
            .build_eframe(|cc| {
                let library = MockLibrary::new();
                let player = Box::new(MockPlayer::playing_demo(&library.tracks()[0]));
                App::for_run(cc, Box::new(library), player, true)
            });
        // A couple of frames fold the mock library's synthetic folders into
        // the in-memory state; shutdown is when a persistent app writes.
        harness.run_steps(2);
        eframe::App::on_exit(harness.state_mut(), None);
    }));

    if built.is_err() {
        eprintln!("skipping: could not build a headless mock app harness here");
        return;
    }

    let after = fs::read(&path).ok();
    // Best effort, before asserting: if a buggy run created the file, don't
    // leave mock garbage behind. Never delete a file that existed before.
    if before.is_none() && after.is_some() {
        let _ = fs::remove_file(&path);
    }
    assert_eq!(
        before,
        after,
        "--mock run modified {} (its bytes changed, or it was created/deleted)",
        path.display()
    );
}
