//! Integration-style tests for the library index.

use std::path::PathBuf;
#[cfg(not(debug_assertions))]
use std::time::Instant;

use emusic_core::{ArtSource, Track, TrackId, TrackKind};

use super::*;

fn track(id: i64, path: &str, artist: &str, album: &str, genre: &str) -> Track {
    let path = PathBuf::from(path);
    let dir = path.parent().unwrap().to_path_buf();
    Track {
        id: TrackId(id),
        path: path.clone(),
        dir,
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
        album_artist: None,
        album: Some(album.to_string()),
        genre: Some(genre.to_string()),
        year: Some(2020),
        track_no: None,
        disc_no: None,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: 1,
    }
}

#[test]
fn build_then_lookup() {
    let tracks = vec![track(1, r"C:\m\a.flac", "A", "X", "Rock")];
    let index = LibraryIndex::build(tracks);
    assert_eq!(index.track_count(), 1);
    assert_eq!(
        index.track(TrackId(1)).unwrap().artist.as_deref(),
        Some("A")
    );
}

#[test]
fn apply_upsert_and_delete() {
    let tracks = vec![track(1, r"C:\m\a.flac", "A", "X", "Rock")];
    let mut index = LibraryIndex::build(tracks);

    index.apply(
        vec![track(2, r"C:\m\b.flac", "B", "Y", "Jazz")],
        &[TrackId(1)],
    );

    assert!(index.track(TrackId(1)).is_none());
    assert_eq!(index.track_count(), 1);
    assert_eq!(index.artists().len(), 1);
    assert_eq!(index.artists()[0].name, "B");
}

#[test]
fn totals_match_tracks() {
    let tracks = vec![
        track(1, r"C:\m\a.flac", "A", "X", "Rock"),
        track(2, r"C:\m\b.flac", "B", "Y", "Jazz"),
    ];
    let index = LibraryIndex::build(tracks);
    assert_eq!(index.totals().track_count, 2);
    assert_eq!(index.totals().duration_ms, 120_000);
}

#[test]
fn empty_index_has_no_groupings() {
    let index = LibraryIndex::new();
    assert!(index.artists().is_empty());
    assert!(index.albums().is_empty());
    assert!(index.genres().is_empty());
    assert!(index.root_dirs().is_empty());
}

#[test]
#[cfg(not(debug_assertions))]
fn build_100k_synthetic_tracks_under_200ms() {
    let tracks = synthetic_tracks(100_000);
    let start = Instant::now();
    let index = LibraryIndex::build(tracks);
    let elapsed = start.elapsed();

    assert_eq!(index.track_count(), 100_000);
    assert!(
        elapsed.as_millis() < 200,
        "building 100k tracks took {elapsed:?}, expected < 200 ms"
    );
}

#[cfg(not(debug_assertions))]
fn synthetic_tracks(count: usize) -> Vec<Track> {
    let mut tracks = Vec::with_capacity(count);
    for i in 0..count {
        let id = (i + 1) as i64;
        let artist_no = i % 500;
        let album_no = i % 2_000;
        let genre_no = i % 50;
        let path = PathBuf::from(format!(
            r"C:\music\Artist{artist_no}\Album{album_no}\{i:06}.flac"
        ));
        let dir = path.parent().unwrap().to_path_buf();
        tracks.push(Track {
            id: TrackId(id),
            path: path.clone(),
            dir,
            filename: format!("{i:06}.flac"),
            ext: "flac".to_string(),
            size: 1_000_000,
            mtime: 1_700_000_000,
            kind: TrackKind::Stream,
            duration_ms: 180_000,
            bitrate: Some(900),
            sample_rate: Some(44_100),
            channels: Some(2),
            title: Some(format!("Track {i}")),
            artist: Some(format!("Artist {artist_no}")),
            album_artist: None,
            album: Some(format!("Album {album_no}")),
            genre: Some(format!("Genre {genre_no}")),
            year: Some(2020 + (i % 10) as i32),
            track_no: Some((i % 20 + 1) as u32),
            disc_no: None,
            composer: None,
            comment: None,
            art_source: ArtSource::None,
            added_at: 1_700_000_000,
        });
    }
    tracks
}
