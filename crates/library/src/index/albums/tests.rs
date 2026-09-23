//! Tests for the album grouping.

use std::path::PathBuf;

use emusic_core::{ArtSource, Track, TrackId, TrackKind};

use super::*;

fn track(artist: &str, album_artist: Option<&str>, album: &str) -> Track {
    Track {
        id: TrackId::UNASSIGNED,
        path: PathBuf::from(r"C:\music\t.flac"),
        dir: PathBuf::from(r"C:\music"),
        filename: "t.flac".to_string(),
        ext: "flac".to_string(),
        size: 1,
        mtime: 1,
        kind: TrackKind::Stream,
        duration_ms: 60_000,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: None,
        artist: Some(artist.to_string()),
        album_artist: album_artist.map(ToString::to_string),
        album: Some(album.to_string()),
        genre: None,
        year: Some(2020),
        track_no: None,
        disc_no: None,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: 1,
        starred: false,
    }
}

#[test]
fn groups_by_artist_and_album() {
    let mut albums = Albums::default();
    albums.add(&track("A", None, "X"), 0);
    albums.add(&track("A", None, "Y"), 1);
    let built = albums.build();
    assert_eq!(built.len(), 2);
}

#[test]
fn promotes_to_various_artists() {
    let mut albums = Albums::default();
    albums.add(&track("A1", None, "Comp"), 0);
    albums.add(&track("A2", None, "Comp"), 1);
    albums.add(&track("A3", None, "Comp"), 2);
    albums.add(&track("A4", None, "Comp"), 3);
    let built = albums.build();
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].artist, VARIOUS_ARTISTS);
    assert_eq!(built[0].track_count(), 4);
}

#[test]
fn respects_explicit_various_artists() {
    let mut albums = Albums::default();
    albums.add(&track("A1", Some(VARIOUS_ARTISTS), "Comp"), 0);
    albums.add(&track("A2", Some(VARIOUS_ARTISTS), "Comp"), 1);
    let built = albums.build();
    assert_eq!(built[0].artist, VARIOUS_ARTISTS);
}

#[test]
fn aggregates_year_and_duration() {
    let mut albums = Albums::default();
    albums.add(&track("A", None, "X"), 0);
    let built = albums.build();
    assert_eq!(built[0].year, Some(2020));
    assert_eq!(built[0].duration_ms, 60_000);
}
