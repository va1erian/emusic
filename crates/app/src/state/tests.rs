//! Unit tests for the [`Accent`] config/CLI spellings: preset names,
//! `#rrggbb` custom colours, and rejection of garbage.

use eframe::egui::Color32;

use crate::state::Accent;

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
