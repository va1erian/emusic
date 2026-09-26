#![forbid(unsafe_code)]

//! Standalone harness for the projectM surface (#301): hosts [`ProjectMView`]
//! full-window and ticks it against the mock player.
//!
//! ```text
//! cargo run -p emusic --example projectm
//! ```
//!
//! With the projectM DLLs installed next to the executable (see #298) it
//! renders presets; without them it shows the fallback plasma and the
//! "projectM not installed" hint. Close the window to quit.

use emusic::app::Msg;
use emusic::views::projectm::ProjectMView;
use emusic_ui::mock::MockPlayer;
use xui::column;
use xui::prelude::*;

/// The harness app: the surface, the mock player it is fed from, and the
/// repaint timer.
struct Demo {
    view: ProjectMView,
    player: MockPlayer,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Timer => self.view.feed(&self.player),
            Msg::Quit => ui.close(),
            _ => {}
        }
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .with_writer(std::io::stderr)
        .init();
    xui::run_app(
        WindowSpec::new("emusic projectM").theme(Theme::dark()),
        |ui| {
            let view = ProjectMView::new(ui).expect("create projectM view");
            ui.set_layout(column![view.fill(1)]);
            ui.on_timer(|_| Some(Msg::Timer));
            ui.on_close(|| Some(Msg::Quit));
            let _ = ui.set_timer(16);
            Demo {
                view,
                player: MockPlayer::default(),
            }
        },
    )?;
    Ok(())
}
