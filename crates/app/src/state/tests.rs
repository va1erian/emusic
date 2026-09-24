//! Unit tests for the navigation commands applied to
//! [`AppState::apply_local`]. The [`Accent`] spelling tests moved to
//! `emusic-ui` with the code (#94).

use crate::state::{AppState, Command, View};

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
