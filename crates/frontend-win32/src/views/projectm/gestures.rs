#![forbid(unsafe_code)]

//! What each projectM surface does with a [`ProjectMGesture`] (#302, #303,
//! #515). A double-click always moves the visualization to the *other* dock:
//! the panel pops it out into its own window, the window docks it back.

use emusic_ui::state::projectm::{VizCommand, VizDock};

use super::ProjectMGesture;
use crate::app::Msg;

/// The main-app message for a gesture on the docked (panel) surface.
pub(crate) fn panel_gesture(gesture: ProjectMGesture) -> Option<Msg> {
    Some(surface_gesture(gesture, VizDock::Window))
}

/// The main-app message for a gesture on the independent window's surface.
pub(super) fn window_gesture(gesture: ProjectMGesture) -> Msg {
    surface_gesture(gesture, VizDock::Panel)
}

/// Maps a gesture on a surface whose other dock is `other`: "pop out" and a
/// double-click move it there, the hide button hides it, and a right-click
/// opens the preset menu.
fn surface_gesture(gesture: ProjectMGesture, other: VizDock) -> Msg {
    match gesture {
        ProjectMGesture::PopOut | ProjectMGesture::DoubleClick => {
            Msg::Viz(VizCommand::SetDock(other))
        }
        ProjectMGesture::Hide => Msg::Viz(VizCommand::SetVisible(false)),
        ProjectMGesture::ContextMenu => Msg::VizMenu,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_click_on_the_panel_pops_it_out() {
        assert!(matches!(
            panel_gesture(ProjectMGesture::DoubleClick),
            Some(Msg::Viz(VizCommand::SetDock(VizDock::Window)))
        ));
    }

    #[test]
    fn double_click_on_the_window_docks_it_back() {
        assert!(matches!(
            window_gesture(ProjectMGesture::DoubleClick),
            Msg::Viz(VizCommand::SetDock(VizDock::Panel))
        ));
    }

    #[test]
    fn hide_and_context_menu_are_the_same_on_both_surfaces() {
        assert!(matches!(
            panel_gesture(ProjectMGesture::Hide),
            Some(Msg::Viz(VizCommand::SetVisible(false)))
        ));
        assert!(matches!(
            window_gesture(ProjectMGesture::Hide),
            Msg::Viz(VizCommand::SetVisible(false))
        ));
        assert!(matches!(
            panel_gesture(ProjectMGesture::ContextMenu),
            Some(Msg::VizMenu)
        ));
        assert!(matches!(
            window_gesture(ProjectMGesture::ContextMenu),
            Msg::VizMenu
        ));
    }
}
