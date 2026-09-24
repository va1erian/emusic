//! The main window's menu bar (#106): the same entries as the egui frontend's,
//! mapped to [`Msg`] like every other win32ui widget.

use emusic_ui::state::{Command, PanelKind, View};
use win32ui::Menu;
use win32ui::prelude::*;

use crate::app::Msg;

/// Builds the File/View menu bar.
#[must_use]
pub fn build() -> Menu<Msg> {
    let file = Menu::new()
        .item("Database info...", None, || Msg::DatabaseInfo)
        .item("Settings", None, || {
            Msg::Dispatch(Command::SetView(View::Settings))
        })
        .separator()
        .item("Quit", Shortcut::ctrl(Key::Q), || Msg::Quit);

    let view = Menu::new()
        .item("Navigator", None, || {
            Msg::Dispatch(Command::TogglePanel(PanelKind::Navigator))
        })
        .item("Now playing panel", None, || {
            Msg::Dispatch(Command::TogglePanel(PanelKind::RightPanel))
        })
        .item("Status bar", None, || {
            Msg::Dispatch(Command::TogglePanel(PanelKind::StatusBar))
        })
        .separator()
        .item("Column browser", None, || {
            Msg::Dispatch(Command::ToggleColumnBrowser)
        })
        .separator()
        .item("Toggle dark / light theme", None, || {
            Msg::Dispatch(Command::ToggleTheme)
        });

    Menu::new().submenu("&File", file).submenu("&View", view)
}
