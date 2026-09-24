//! The main window's menu bar (#106, #262): the same entries as the egui
//! frontend's, mapped to [`Msg`] like every other win32ui widget. The View
//! toggles are checkable, so the menu shows a tick for the panels and the
//! column browser that are currently on.

use emusic_ui::state::{AppState, Command, PanelKind, View};
use win32ui::Menu;
use win32ui::prelude::*;

use crate::app::Msg;

/// Builds the File/View menu bar, ticking the View toggles that are on.
///
/// `win32ui` has no runtime checked-state setter, so the caller reinstalls the
/// bar with a freshly built one whenever the visibility state changes.
#[must_use]
pub fn build(state: &AppState) -> Menu<Msg> {
    let file = Menu::new()
        .item("Database info...", None, || Msg::DatabaseInfo)
        .item("Settings", None, || {
            Msg::Dispatch(Command::SetView(View::Settings))
        })
        .separator()
        .item("Quit", Shortcut::ctrl(Key::Q), || Msg::Quit);

    let view = Menu::new()
        .checked_item("Navigator", None, state.panels.navigator, || {
            Msg::Dispatch(Command::TogglePanel(PanelKind::Navigator))
        })
        .checked_item("Now playing panel", None, state.panels.right_panel, || {
            Msg::Dispatch(Command::TogglePanel(PanelKind::RightPanel))
        })
        .checked_item("Status bar", None, state.panels.status_bar, || {
            Msg::Dispatch(Command::TogglePanel(PanelKind::StatusBar))
        })
        .separator()
        .checked_item("Column browser", None, state.music.browser.visible, || {
            Msg::Dispatch(Command::ToggleColumnBrowser)
        })
        .separator()
        .item("Toggle dark / light theme", None, || {
            Msg::Dispatch(Command::ToggleTheme)
        });

    Menu::new().submenu("&File", file).submenu("&View", view)
}
