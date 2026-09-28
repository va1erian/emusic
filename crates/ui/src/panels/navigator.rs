//! Navigator model (#104): the sidebar's sections of views and the intent to
//! switch the central view.
//!
//! The app renders the same [`SECTIONS`], adding a per-view
//! icon, followed by a dynamic PLAYLISTS section (#473) built from
//! [`Navigator::playlists`]. Clicks map to [`NavigatorMsg`].

use crate::library_api::PlaylistInfo;
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
    View::Visualization,
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
    /// Show the playlist with this id (#473).
    SelectPlaylist(u64),
}

/// Applies a navigator intent, queueing the shell [`Command`]s it produces.
pub fn update(msg: NavigatorMsg, out: &mut Commands) {
    match msg {
        NavigatorMsg::Select(view) => out.push(Command::SetView(view)),
        NavigatorMsg::SelectPlaylist(id) => out.push(Command::SelectPlaylist(id)),
    }
}

/// The heading of the dynamic playlists section (#473).
pub const PLAYLISTS_HEADING: &str = "PLAYLISTS";

/// A name for a new playlist that no existing playlist uses: "New playlist",
/// then "New playlist 2", "New playlist 3", ...
pub fn new_playlist_name(existing: &[PlaylistInfo]) -> String {
    const BASE: &str = "New playlist";
    let taken = |name: &str| existing.iter().any(|playlist| playlist.name == name);
    if !taken(BASE) {
        return BASE.to_owned();
    }
    (2..)
        .map(|n| format!("{BASE} {n}"))
        .find(|name| !taken(name))
        .expect("an unbounded range always yields a free name")
}

/// The navigator's selected-view state, so the retained-mode frontend knows
/// when the highlight must be redrawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Navigator {
    selected: View,
    /// The playlists listed under [`PLAYLISTS_HEADING`] (#473).
    playlists: Vec<PlaylistInfo>,
    /// The playlist highlighted while [`Navigator::selected`] is
    /// [`View::Playlist`].
    selected_playlist: Option<u64>,
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

    /// Adopts the library's playlists and the shell's selected playlist,
    /// bumping [`Navigator::revision`] when either changed (#473).
    pub fn sync_playlists(&mut self, playlists: &[PlaylistInfo], selected: Option<u64>) {
        if self.playlists != playlists || self.selected_playlist != selected {
            self.playlists = playlists.to_vec();
            self.selected_playlist = selected;
            self.revision += 1;
        }
    }

    /// The playlists to list under [`PLAYLISTS_HEADING`], in order.
    pub fn playlists(&self) -> &[PlaylistInfo] {
        &self.playlists
    }

    /// The playlist row to highlight: the selected playlist while the
    /// Playlist view is shown, otherwise none.
    pub fn highlighted_playlist(&self) -> Option<u64> {
        (self.selected == View::Playlist)
            .then_some(self.selected_playlist)
            .flatten()
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
        // Settings is reached from the menu and Playlist from the dynamic
        // playlists section, so they are the only views not in `SECTIONS`.
        assert_eq!(listed.len(), View::ALL.len() - 2);
        assert!(!listed.contains(&View::Playlist));
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

    fn playlist(id: u64, name: &str) -> PlaylistInfo {
        PlaylistInfo {
            id,
            name: name.into(),
            track_count: 0,
        }
    }

    #[test]
    fn sync_playlists_bumps_only_on_change_and_highlights_when_shown() {
        let mut navigator = Navigator::default();
        let lists = [playlist(1, "A"), playlist(2, "B")];
        navigator.sync_playlists(&lists, Some(2));
        assert_eq!(navigator.revision(), 1);
        navigator.sync_playlists(&lists, Some(2));
        assert_eq!(navigator.revision(), 1, "unchanged playlists don't bump");
        assert_eq!(navigator.playlists().len(), 2);

        assert_eq!(navigator.highlighted_playlist(), None, "not on the view");
        navigator.sync(View::Playlist);
        assert_eq!(navigator.highlighted_playlist(), Some(2));
    }

    #[test]
    fn new_playlist_names_are_unique() {
        assert_eq!(new_playlist_name(&[]), "New playlist");
        let one = [playlist(1, "New playlist")];
        assert_eq!(new_playlist_name(&one), "New playlist 2");
        let two = [playlist(1, "New playlist"), playlist(2, "New playlist 2")];
        assert_eq!(new_playlist_name(&two), "New playlist 3");
        let other = [playlist(1, "Road trip")];
        assert_eq!(new_playlist_name(&other), "New playlist");
    }

    #[test]
    fn select_playlist_emits_select_playlist() {
        let mut out = Commands::new();
        update(NavigatorMsg::SelectPlaylist(5), &mut out);
        assert_eq!(out.into_vec(), vec![Command::SelectPlaylist(5)]);
    }

    #[test]
    fn select_emits_set_view() {
        let mut out = Commands::new();
        update(NavigatorMsg::Select(View::History), &mut out);
        assert_eq!(out.into_vec(), vec![Command::SetView(View::History)]);
    }
}
