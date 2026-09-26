//! The main window's menu bar (#106, #262): the app's top-level entries,
//! mapped to [`Msg`] like every other win32ui widget. The View toggles are
//! checkable, so the menu shows a tick for the panels and the column browser
//! that are currently on. The View → Visualization submenu (#306) shows the
//! projectM preset actions and placement; [`viz_context`] is the surface's
//! right-click menu.

use emusic_ui::state::projectm::{PresetRequest, VizDock};
use emusic_ui::state::{AppState, Command, PanelKind, View, VizCommand};
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
        .checked_item("Next tracks", None, state.panels.next_tracks, || {
            Msg::Dispatch(Command::TogglePanel(PanelKind::NextTracks))
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
        })
        .separator()
        .submenu("&Visualization", viz_menu(state));

    Menu::new().submenu("&File", file).submenu("&View", view)
}

/// The View → Visualization submenu: show/hide, preset navigation and lock,
/// and the placement radio group (#306). Its ticks are updated in place from
/// the shell tick through [`Ui::set_menu_checked`](win32ui::Ui::set_menu_checked).
fn viz_menu(state: &AppState) -> Menu<Msg> {
    let layout = &state.projectm.layout;
    let preset = Menu::new()
        .item("Next", None, || {
            Msg::Viz(VizCommand::Preset(PresetRequest::Next))
        })
        .item("Previous", None, || {
            Msg::Viz(VizCommand::Preset(PresetRequest::Previous))
        })
        .item("Random", None, || {
            Msg::Viz(VizCommand::Preset(PresetRequest::Random))
        })
        .separator()
        .checked_item(
            "Lock current preset",
            None,
            state.projectm.settings.preset_locked,
            || Msg::Viz(VizCommand::TogglePresetLock),
        )
        .keyed("viz-lock");

    let placement = Menu::new()
        .radio_item(
            "Panel",
            None,
            !layout.fullscreen && layout.dock == VizDock::Panel,
            || Msg::Viz(VizCommand::SetDock(VizDock::Panel)),
        )
        .keyed("viz-panel")
        .radio_item(
            "Window",
            None,
            !layout.fullscreen && layout.dock == VizDock::Window,
            || Msg::Viz(VizCommand::SetDock(VizDock::Window)),
        )
        .keyed("viz-window")
        .radio_item("Fullscreen", None, layout.fullscreen, || {
            Msg::Viz(VizCommand::SetFullscreen(true))
        })
        .keyed("viz-fullscreen");

    Menu::new()
        .checked_item("Show visualization", None, layout.visible, || {
            Msg::Viz(VizCommand::ToggleVisible)
        })
        .keyed("viz-show")
        .separator()
        .submenu("Preset", preset)
        .submenu("Placement", placement)
}

/// The projectM surface's right-click menu (#306): the preset actions plus
/// hiding the surface.
#[must_use]
pub fn viz_context(state: &AppState) -> Menu<Msg> {
    Menu::new()
        .item("Next preset", None, || {
            Msg::Viz(VizCommand::Preset(PresetRequest::Next))
        })
        .item("Previous preset", None, || {
            Msg::Viz(VizCommand::Preset(PresetRequest::Previous))
        })
        .item("Random preset", None, || {
            Msg::Viz(VizCommand::Preset(PresetRequest::Random))
        })
        .separator()
        .checked_item(
            "Lock current preset",
            None,
            state.projectm.settings.preset_locked,
            || Msg::Viz(VizCommand::TogglePresetLock),
        )
        .separator()
        .item("Hide", None, || Msg::Viz(VizCommand::SetVisible(false)))
}
