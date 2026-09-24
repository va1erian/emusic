//! Navigator model (#104): the sidebar's sections of views and the intent to
//! switch the central view.
//!
//! Both frontends render the same [`SECTIONS`]; the win32 one adds a per-view
//! icon. Clicks map to [`NavigatorMsg`].

use crate::state::{Command, View};
use crate::views::Commands;

/// One navigator section: a heading and the views listed under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    /// The small section heading, e.g. `LIBRARY`.
    pub heading: &'static str,
    /// The views in the section, in order.
    pub views: &'static [View],
}

/// The library-section views, in order.
pub const LIBRARY_VIEWS: &[View] = &[
    View::Music,
    View::Albums,
    View::Artists,
    View::Genres,
    View::Folders,
];
/// The activity-section views, in order.
pub const ACTIVITY_VIEWS: &[View] = &[
    View::Starred,
    View::MostPlayed,
    View::History,
    View::NowPlaying,
];

/// The navigator's sections, in order.
pub const SECTIONS: &[Section] = &[
    Section {
        heading: "LIBRARY",
        views: LIBRARY_VIEWS,
    },
    Section {
        heading: "ACTIVITY",
        views: ACTIVITY_VIEWS,
    },
];

/// A navigator intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigatorMsg {
    /// Switch the central view to this view.
    Select(View),
}

/// Applies a navigator intent, queueing the shell [`Command`]s it produces.
pub fn update(msg: NavigatorMsg, out: &mut Commands) {
    match msg {
        NavigatorMsg::Select(view) => out.push(Command::SetView(view)),
    }
}

/// The navigator's selected-view state, so the retained-mode frontend knows
/// when the highlight must be redrawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Navigator {
    selected: View,
    revision: u64,
}

impl Navigator {
    /// Adopts the shell's current view, bumping [`Navigator::revision`] when it
    /// changed.
    pub fn sync(&mut self, view: View) {
        if self.selected != view {
            self.selected = view;
            self.revision += 1;
        }
    }

    /// The view currently highlighted in the sidebar.
    pub fn selected(&self) -> View {
        self.selected
    }

    /// The revision counter, bumped whenever the selection changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_view_appears_exactly_once() {
        let listed: Vec<View> = SECTIONS
            .iter()
            .flat_map(|section| section.views.iter().copied())
            .collect();
        for &view in &listed {
            assert_eq!(
                listed.iter().filter(|&&other| other == view).count(),
                1,
                "{view:?} appears more than once"
            );
        }
        // Settings is reached from the menu, not the navigator, so it is the
        // only view not listed.
        assert_eq!(listed.len(), View::ALL.len() - 1);
    }

    #[test]
    fn sync_tracks_the_selection_and_revision() {
        let mut navigator = Navigator::default();
        assert_eq!(navigator.selected(), View::Music);
        navigator.sync(View::Music);
        assert_eq!(navigator.revision(), 0, "unchanged selection doesn't bump");
        navigator.sync(View::Albums);
        assert_eq!(navigator.selected(), View::Albums);
        assert_eq!(navigator.revision(), 1);
    }

    #[test]
    fn select_emits_set_view() {
        let mut out = Commands::new();
        update(NavigatorMsg::Select(View::History), &mut out);
        assert_eq!(out.into_vec(), vec![Command::SetView(View::History)]);
    }
}
