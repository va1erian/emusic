//! Harness for the projectM surface (#301): hosts the real [`ProjectMView`] in
//! a window and ticks it against a mock player. Without the projectM DLLs it
//! exercises the placeholder/fallback path; with them installed, the GL path.
//! Skips (prints and returns) if the session cannot create windows.

#![cfg(windows)]

use std::cell::RefCell;
use std::rc::Rc;

use emusic::app::Msg;
use emusic::views::projectm::ProjectMView;
use emusic_ui::mock::MockPlayer;
use emusic_ui::state::projectm::ProjectMAvailability;
use xui::column;
use xui::prelude::*;

/// How many animation ticks the harness runs before quitting.
const TICKS: u32 = 30;

/// The harness app: the surface plus the mock player it is fed from.
struct Harness {
    view: ProjectMView,
    player: MockPlayer,
    ticks: u32,
    seen: Rc<RefCell<Option<ProjectMAvailability>>>,
}

impl App for Harness {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Timer => {
                self.view.feed(&self.player);
                self.ticks += 1;
                if self.ticks >= TICKS {
                    *self.seen.borrow_mut() = Some(self.view.availability());
                    ui.close();
                }
            }
            Msg::Quit => ui.close(),
            _ => {}
        }
    }
}

#[test]
fn projectm_surface_paints_and_ticks() {
    let seen = Rc::new(RefCell::new(None));
    let seen_for_make = Rc::clone(&seen);

    let result = xui::run_app(
        WindowSpec::new("emusic.projectm").theme(Theme::dark()),
        move |ui| {
            let view = ProjectMView::new(ui).expect("create projectM view");
            ui.set_layout(column![view.fill(1)]);
            ui.on_timer(|_| Some(Msg::Timer));
            let _ = ui.set_timer(16);

            Harness {
                view,
                player: MockPlayer::default(),
                ticks: 0,
                seen: seen_for_make,
            }
        },
    );

    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return;
    }

    let seen = seen.borrow().clone();
    assert!(
        matches!(
            seen,
            Some(
                ProjectMAvailability::Available(_)
                    | ProjectMAvailability::MissingLibrary
                    | ProjectMAvailability::NoOpenGl
            )
        ),
        "the surface never resolved its engine status: {seen:?}"
    );
}
