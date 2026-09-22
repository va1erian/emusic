use std::path::PathBuf;

use super::*;

fn track_with_meta(
    artist: Option<&str>,
    album: Option<&str>,
    album_artist: Option<&str>,
    title: Option<&str>,
) -> emusic_core::Track {
    emusic_core::Track {
        id: emusic_core::TrackId::UNASSIGNED,
        path: PathBuf::from(r"C:\músic\Artist\Album\01 Song.flac"),
        dir: PathBuf::from(r"C:\músic\Artist\Album"),
        filename: "01 Song.flac".to_string(),
        ext: "flac".to_string(),
        size: 0,
        mtime: 0,
        kind: emusic_core::TrackKind::Stream,
        duration_ms: 0,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: title.map(str::to_owned),
        artist: artist.map(str::to_owned),
        album_artist: album_artist.map(str::to_owned),
        album: album.map(str::to_owned),
        genre: None,
        year: None,
        track_no: None,
        disc_no: None,
        composer: None,
        comment: None,
        art_source: emusic_core::ArtSource::None,
        added_at: 0,
    }
}

#[test]
fn full_haystack_contains_all_fields() {
    let track = track_with_meta(
        Some("Daft Punk"),
        Some("Discovery"),
        Some("Daft Punk"),
        Some("One More Time"),
    );
    let haystack = Haystack::build(&track);
    let full = haystack.full();
    assert!(full.contains("daft punk"));
    assert!(full.contains("discovery"));
    assert!(full.contains("one more time"));
}

#[test]
fn diacritics_are_folded() {
    let track = track_with_meta(Some("Björk"), None, None, None);
    let haystack = Haystack::build(&track);
    assert!(haystack.full().contains("bjork"));
}

#[test]
fn field_slice_limits_search_to_field() {
    let track = track_with_meta(Some("Artist"), Some("Album"), None, Some("Title"));
    let haystack = Haystack::build(&track);
    assert!(haystack.field(Field::Artist).contains("artist"));
    assert!(haystack.field(Field::Album).contains("album"));
    assert!(haystack.field(Field::Title).contains("title"));
    assert!(!haystack.field(Field::Artist).contains("album"));
}

#[test]
fn album_artist_falls_back_to_artist() {
    let track = track_with_meta(Some("Band"), None, None, None);
    let haystack = Haystack::build(&track);
    assert!(haystack.field(Field::AlbumArtist).contains("band"));
}

#[test]
fn file_and_dir_fields_are_populated() {
    let track = track_with_meta(None, None, None, None);
    let haystack = Haystack::build(&track);
    assert!(haystack.field(Field::File).contains("01 song.flac"));
    assert!(
        haystack
            .field(Field::Dir)
            .contains(r"c:\music\artist\album")
    );
    assert!(haystack.field(Field::Ext).contains("flac"));
}

#[test]
fn missing_fields_are_empty_slices() {
    let track = track_with_meta(None, None, None, None);
    let haystack = Haystack::build(&track);
    assert!(haystack.field(Field::Artist).is_empty());
    assert!(haystack.field(Field::Title).is_empty());
}

#[test]
fn field_separator_is_sanitized() {
    // A field value containing the separator would corrupt offsets if left
    // untouched. It should be replaced with a space inside the field slice.
    let track = track_with_meta(Some("A\x1fB"), None, None, None);
    let haystack = Haystack::build(&track);
    assert!(haystack.field(Field::Artist).contains("a b"));
    assert!(!haystack.field(Field::Artist).contains('\x1f'));
}

#[test]
fn numeric_fields_return_empty_slice() {
    let track = track_with_meta(None, None, None, None);
    let haystack = Haystack::build(&track);
    assert!(haystack.field(Field::Year).is_empty());
    assert!(haystack.field(Field::Duration).is_empty());
}
