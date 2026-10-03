//! Unit tests for the navigation commands applied to
//! [`AppState::apply_local`]. The [`Accent`] spelling tests live alongside
//! the appearance code (#94).

use crate::state::{AppState, Command, SettingsTab, View};

#[test]
fn go_to_artist_switches_view_and_seeds_the_search() {
    let mut state = AppState::default();
    state.apply_local(&Command::GoToArtist("Boards of Canada".to_string()));
    assert_eq!(state.view, View::Artists);
    assert_eq!(state.search_query, "Boards of Canada");
}

#[test]
fn go_to_album_switches_view_and_selects_the_album() {
    let mut state = AppState::default();
    state.apply_local(&Command::GoToAlbum {
        name: "Geogaddi".to_string(),
        artist: "Boards of Canada".to_string(),
    });
    assert_eq!(state.view, View::Albums);
    assert!(state.album_grid.selected.is_some());
}

#[test]
fn open_settings_selects_the_view_and_tab() {
    let mut state = AppState {
        view: View::Music,
        settings_tab: SettingsTab::Library,
        ..AppState::default()
    };

    state.apply_local(&Command::OpenSettings(SettingsTab::About));
    assert_eq!(state.view, View::Settings);
    assert_eq!(state.settings_tab, SettingsTab::About);
}

#[test]
fn accent_tint_commands_update_the_state() {
    let mut state = AppState::default();
    assert!(!state.accent_tint);

    state.apply_local(&Command::SetAccentTint(true));
    state.apply_local(&Command::SetAccentTintStrength(0xD0));
    assert!(state.accent_tint);
    assert_eq!(state.accent_tint_strength, 0xD0);
}

#[test]
fn docking_into_the_panel_reveals_a_hidden_right_panel() {
    use crate::state::projectm::{VizDock, VizSurface};
    use crate::state::{PanelKind, VizCommand};

    let mut state = AppState::default();
    state.apply_local(&Command::Viz(VizCommand::SetDock(VizDock::Window)));
    state.apply_local(&Command::TogglePanel(PanelKind::RightPanel));
    assert!(!state.panels.right_panel, "hidden while in window mode");

    // A double-click in the window docks it back into the panel (#515).
    state.apply_local(&Command::Viz(VizCommand::SetDock(VizDock::Panel)));
    assert_eq!(state.projectm.surface(), Some(VizSurface::Panel));
    assert!(
        state.panels.right_panel,
        "the docked surface must stay on screen"
    );
}

#[test]
fn showing_the_docked_visualization_reveals_a_hidden_right_panel() {
    use crate::state::{PanelKind, VizCommand};

    let mut state = AppState::default();
    state.apply_local(&Command::TogglePanel(PanelKind::RightPanel));
    state.apply_local(&Command::Viz(VizCommand::SetVisible(true)));
    assert!(state.panels.right_panel);
}

#[test]
fn hiding_the_right_panel_while_docked_stays_hidden() {
    use crate::state::{PanelKind, VizCommand};

    let mut state = AppState::default();
    state.apply_local(&Command::Viz(VizCommand::SetVisible(true)));
    state.apply_local(&Command::TogglePanel(PanelKind::RightPanel));
    // Preset navigation does not move the surface, so it leaves the panel alone.
    state.apply_local(&Command::Viz(VizCommand::TogglePresetLock));
    assert!(!state.panels.right_panel);
}
