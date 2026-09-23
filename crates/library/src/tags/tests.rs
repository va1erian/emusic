//! Round-trip, preservation and error tests for the tag read/write API.
//!
//! Rather than commit a binary fixture, each test synthesizes a small but
//! valid WAV, seeds it with tags through `lofty` (independently of the code
//! under test) and copies that fixture to its own temp file — the issue's
//! "copy a real tagged fixture to a temp file, write, and read back" flow.

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::tag::{ItemKey, Tag, TagExt, TagType};

use super::read::{number_of, year_of};
use super::test_support::{cleanup, tagged_fixture};
use super::{EditableTags, read_tags, write_tags};
use crate::error::LibraryError;

#[test]
fn reads_existing_tags_from_a_file() {
    let path = tagged_fixture("reads-existing");
    let tags = read_tags(&path).unwrap();
    assert_eq!(tags.title.as_deref(), Some("Seed Title"));
    assert_eq!(tags.artist.as_deref(), Some("Seed Artist"));
    assert_eq!(tags.comment.as_deref(), Some("Seed Comment"));
    assert_eq!(tags.album, None);
    cleanup(&path);
}

#[test]
fn round_trips_every_scalar_field() {
    let path = tagged_fixture("round-trip");
    let wanted = EditableTags {
        title: Some("Title".to_string()),
        artist: Some("Artist".to_string()),
        album: Some("Album".to_string()),
        album_artist: Some("Album Artist".to_string()),
        genre: Some("Genre".to_string()),
        year: Some(2021),
        track_no: Some(7),
        disc_no: Some(2),
        composer: Some("Composer".to_string()),
        comment: Some("Comment".to_string()),
    };

    write_tags(&path, &wanted).unwrap();
    assert_eq!(read_tags(&path).unwrap(), wanted);
    cleanup(&path);
}

#[test]
fn writing_none_removes_a_seeded_field() {
    let path = tagged_fixture("clears-field");
    let edited = EditableTags {
        title: Some("Only Title".to_string()),
        ..Default::default()
    };

    write_tags(&path, &edited).unwrap();
    let tags = read_tags(&path).unwrap();
    assert_eq!(tags.title.as_deref(), Some("Only Title"));
    assert_eq!(tags.artist, None);
    assert_eq!(tags.comment, None);
    cleanup(&path);
}

#[test]
fn write_preserves_tag_items_it_does_not_expose() {
    let path = tagged_fixture("preserves-items");

    let mut file = lofty::read_from_path(&path).unwrap();
    file.primary_tag_mut()
        .unwrap()
        .insert_text(ItemKey::AlbumTitleSortOrder, "keep me".to_string());
    file.primary_tag()
        .unwrap()
        .save_to_path(&path, WriteOptions::default())
        .unwrap();

    write_tags(
        &path,
        &EditableTags {
            title: Some("Changed".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    let file = lofty::read_from_path(&path).unwrap();
    let sort_order: Vec<&str> = file
        .primary_tag()
        .unwrap()
        .get_strings(ItemKey::AlbumTitleSortOrder)
        .collect();
    assert_eq!(sort_order, ["keep me"]);
    cleanup(&path);
}

#[test]
fn missing_file_is_an_error() {
    let missing = std::env::temp_dir().join("emusic-tags-missing/does-not-exist.wav");
    assert!(matches!(
        read_tags(&missing),
        Err(LibraryError::ReadTags { .. })
    ));
    assert!(write_tags(&missing, &EditableTags::default()).is_err());
}

#[test]
fn number_of_parses_plain_and_slash_suffixed_forms() {
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::TrackNumber, "3/12".to_string());
    assert_eq!(number_of(&tag, ItemKey::TrackNumber), Some(3));

    tag.insert_text(ItemKey::TrackNumber, "n/a".to_string());
    assert_eq!(number_of(&tag, ItemKey::TrackNumber), None);
}

#[test]
fn year_of_accepts_plain_years_and_full_dates() {
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::RecordingDate, "1999".to_string());
    assert_eq!(year_of(&tag), Some(1999));

    tag.insert_text(ItemKey::RecordingDate, "2021-05-03".to_string());
    assert_eq!(year_of(&tag), Some(2021));
}
