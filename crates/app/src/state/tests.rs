//! Unit tests for the [`Accent`] config/CLI spellings (preset names,
//! `#rrggbb` custom colours, rejection of garbage) and for the navigation
//! commands applied to [`AppState::apply_local`].

use eframe::egui::Color32;

use crate::state::{Accent, AppState, Command, View};

#[test]
fn preset_names_parse_case_insensitively() {
    assert_eq!(Accent::parse("blue"), Some(Accent::Blue));
    assert_eq!(Accent::parse("PURPLE"), Some(Accent::Purple));
    assert_eq!(Accent::parse(" Teal "), Some(Accent::Teal));
    assert_eq!(Accent::parse("orange"), Some(Accent::Orange));
}

#[test]
fn hex_colours_parse_to_custom() {
    assert_eq!(
        Accent::parse("#12abCF"),
        Some(Accent::Custom(Color32::from_rgb(0x12, 0xAB, 0xCF)))
    );
    // The `#` is optional.
    assert_eq!(
        Accent::parse("e87a1e"),
        Some(Accent::Custom(Color32::from_rgb(0xE8, 0x7A, 0x1E)))
    );
}

#[test]
fn garbage_is_rejected() {
    assert_eq!(Accent::parse(""), None);
    assert_eq!(Accent::parse("magenta"), None);
    assert_eq!(Accent::parse("#12345"), None);
    assert_eq!(Accent::parse("#1234567"), None);
    assert_eq!(Accent::parse("#12g45z"), None);
}

#[test]
fn config_spelling_round_trips_through_parse() {
    for accent in [
        Accent::Orange,
        Accent::Blue,
        Accent::Green,
        Accent::Purple,
        Accent::Red,
        Accent::Teal,
        Accent::Custom(Color32::from_rgb(0xCA, 0xFE, 0xBA)),
    ] {
        assert_eq!(Accent::parse(&accent.to_config_str()), Some(accent));
    }
}

#[test]
fn custom_colours_serialize_as_hex() {
    assert_eq!(
        Accent::Custom(Color32::from_rgb(0xCA, 0xFE, 0xBA)).to_config_str(),
        "#cafeba"
    );
    assert_eq!(Accent::Blue.to_config_str(), "blue");
}

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
