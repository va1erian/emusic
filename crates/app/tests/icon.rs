//! Smoke test for the main window's icon (#361): `build.rs` embeds `emusic.rc`
//! into the test binaries, so `icon::install` must be able to load resource id
//! 1 and set it on a real window. Skips if the session cannot create windows.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;

use win32ui::prelude::*;

/// The resource id `build.rs` embeds from `emusic.rc` (see [`emusic::icon`]).
const ICON_RESOURCE_ID: u16 = 1;

struct IconApp;

impl App for IconApp {
    type Msg = ();

    fn update(&mut self, _msg: (), ui: &mut Ui<()>) {
        ui.quit();
    }
}

#[test]
fn the_embedded_icon_installs_on_a_window() {
    let installed = Rc::new(Cell::new(false));
    let installed_for_make = Rc::clone(&installed);

    let result = win32ui::run_app(
        WindowSpec::new("emusic.icon").theme(Theme::light()),
        move |ui| {
            // The same resource `emusic.exe` and `emusic-shot.exe` carry, so a
            // missing one here means every top-level window would silently fall
            // back to the generic system icon.
            assert!(
                Icon::from_resource(ICON_RESOURCE_ID).is_ok(),
                "resource id {ICON_RESOURCE_ID} must be embedded by build.rs"
            );
            emusic::icon::install(ui);
            installed_for_make.set(true);
            ui.emit(());
            IconApp
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }
    assert!(installed.get(), "the icon was never installed");
}
