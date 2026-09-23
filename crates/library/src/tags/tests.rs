//! Round-trip, preservation and error tests for the tag read/write API.
//!
//! Rather than commit a binary fixture, each test synthesizes a small but
//! valid WAV, seeds it with tags through `lofty` (independently of the code
//! under test) and copies that fixture to its own temp file — the issue's
//! "copy a real tagged fixture to a temp file, write, and read back" flow.

use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::tag::{ItemKey, Tag, TagExt, TagType};

use super::read::{number_of, year_of};
use super::{EditableTags, read_tags, write_tags};
use crate::error::LibraryError;

/// A small but valid 44100 Hz mono 16-bit PCM WAV, generated in-memory.
fn minimal_wav() -> Vec<u8> {
    const RATE: u32 = 44_100;
    const SAMPLES: usize = 1024;
    let data_len = (SAMPLES * 2) as u32;
    let byte_rate = RATE * 2;

    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVE");
    out.extend(b"fmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(RATE.to_le_bytes());
    out.extend(byte_rate.to_le_bytes());
    out.extend(2u16.to_le_bytes()); // block align
    out.extend(16u16.to_le_bytes()); // bits per sample
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    out.extend(std::iter::repeat_n(0u8, data_len as usize));
    out
}

/// The unique temp directory a test named `name` may use.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emusic-tags-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes the tagged WAV fixture and returns the copy each test edits.
fn tagged_fixture(name: &str) -> PathBuf {
    let dir = temp_dir(name);
    let fixture = dir.join("fixture.wav");
    std::fs::write(&fixture, minimal_wav()).unwrap();

    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::TrackTitle, "Seed Title".to_string());
    tag.insert_text(ItemKey::TrackArtist, "Seed Artist".to_string());
    tag.insert_text(ItemKey::Comment, "Seed Comment".to_string());
    tag.save_to_path(&fixture, WriteOptions::default()).unwrap();

    let target = dir.join("track.wav");
    std::fs::copy(&fixture, &target).unwrap();
    target
}

/// Removes the temp directory a fixture/test file lives in.
fn cleanup(path: &Path) {
    if let Some(dir) = path.parent() {
        std::fs::remove_dir_all(dir).ok();
    }
}

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
