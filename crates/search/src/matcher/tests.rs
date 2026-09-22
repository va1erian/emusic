use std::path::PathBuf;

use super::*;
use crate::parse;

fn sample_track(id: i64, artist: &str, title: &str, year: i32) -> Track {
    Track {
        id: TrackId(id),
        path: PathBuf::from(format!(r"C:\music\{}\{}.flac", artist, title)),
        dir: PathBuf::from(format!(r"C:\music\{}", artist)),
        filename: format!("{}.flac", title),
        ext: "flac".to_string(),
        size: 0,
        mtime: 0,
        kind: emusic_core::TrackKind::Stream,
        duration_ms: 180_000,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: Some(title.to_string()),
        artist: Some(artist.to_string()),
        album_artist: None,
        album: Some("Album".to_string()),
        genre: Some("Rock".to_string()),
        year: Some(year),
        track_no: None,
        disc_no: None,
        composer: None,
        comment: None,
        art_source: emusic_core::ArtSource::None,
        added_at: 0,
    }
}

fn make_index() -> Index {
    let mut index = Index::new();
    index.add_track(sample_track(1, "Daft Punk", "One More Time", 2001));
    index.add_track(sample_track(2, "The Beatles", "Come Together", 1969));
    index.add_track(sample_track(3, "Daft Punk", "Digital Love", 2001));
    index.add_track(sample_track(4, "Nirvana", "Smells Like Teen Spirit", 1991));
    index
}

#[test]
fn empty_query_returns_all_candidates() {
    let index = make_index();
    let query = parse("");
    let candidates = vec![TrackId(2), TrackId(1), TrackId(99)];
    assert_eq!(
        index.search(&query, &|_| 0u64, &candidates),
        vec![TrackId(2), TrackId(1), TrackId(99)]
    );
}

#[test]
fn text_term_matches_any_field() {
    let index = make_index();
    let query = parse("punk");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1), TrackId(2), TrackId(3)]);
    assert_eq!(result, vec![TrackId(1), TrackId(3)]);
}

#[test]
fn field_term_limits_to_field() {
    let index = make_index();
    let query = parse("artist:daft");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1), TrackId(2), TrackId(3)]);
    assert_eq!(result, vec![TrackId(1), TrackId(3)]);
}

#[test]
fn phrase_term_matches_substring() {
    let index = make_index();
    let query = parse("\"one more\"");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1), TrackId(2)]);
    assert_eq!(result, vec![TrackId(1)]);
}

#[test]
fn negated_term_excludes_matches() {
    let index = make_index();
    let query = parse("daft -love");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1), TrackId(2), TrackId(3)]);
    // Track 3 has "Digital Love", so it is excluded.
    assert_eq!(result, vec![TrackId(1)]);
}

#[test]
fn negated_field_term_excludes_matches() {
    let index = make_index();
    let query = parse("-artist:beatles");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1), TrackId(2), TrackId(3)]);
    assert_eq!(result, vec![TrackId(1), TrackId(3)]);
}

#[test]
fn year_range_filter() {
    let index = make_index();
    let query = parse("year:1990..2000");
    let result = index.search(
        &query,
        &|_| 0u64,
        &[TrackId(1), TrackId(2), TrackId(3), TrackId(4)],
    );
    assert_eq!(result, vec![TrackId(4)]);
}

#[test]
fn year_comparison_filter() {
    let index = make_index();
    let query = parse("year:>1990");
    let result = index.search(
        &query,
        &|_| 0u64,
        &[TrackId(1), TrackId(2), TrackId(3), TrackId(4)],
    );
    assert_eq!(result, vec![TrackId(1), TrackId(3), TrackId(4)]);
}

#[test]
fn duration_filter_converts_ms_to_seconds() {
    let mut index = Index::new();
    let mut short = sample_track(1, "A", "Short", 2020);
    short.duration_ms = 90_000;
    let mut long = sample_track(2, "B", "Long", 2020);
    long.duration_ms = 300_000;
    index.add_track(short);
    index.add_track(long);

    let query = parse("duration:>2m");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1), TrackId(2)]);
    assert_eq!(result, vec![TrackId(2)]);
}

#[test]
fn plays_filter_uses_stats_trait() {
    let index = make_index();
    let query = parse("plays:>5");
    let stats = |id: TrackId| -> u64 {
        match id.0 {
            1 => 10,
            2 => 3,
            3 => 7,
            4 => 5,
            _ => 0,
        }
    };
    let result = index.search(
        &query,
        &stats,
        &[TrackId(1), TrackId(2), TrackId(3), TrackId(4)],
    );
    assert_eq!(result, vec![TrackId(1), TrackId(3)]);
}

#[test]
fn missing_numeric_field_does_not_match_positive_filter() {
    let mut index = Index::new();
    let mut track = sample_track(1, "A", "T", 2020);
    track.year = None;
    index.add_track(track);

    let query = parse("year:1990");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1)]);
    assert!(result.is_empty());
}

#[test]
fn missing_numeric_field_matches_negative_filter() {
    let mut index = Index::new();
    let mut track = sample_track(1, "A", "T", 2020);
    track.year = None;
    index.add_track(track);

    let query = parse("-year:1990");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1)]);
    assert_eq!(result, vec![TrackId(1)]);
}

#[test]
fn unknown_track_id_is_skipped() {
    let index = make_index();
    let query = parse("daft");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1), TrackId(99), TrackId(3)]);
    assert_eq!(result, vec![TrackId(1), TrackId(3)]);
}

#[test]
fn complex_query_with_text_and_numeric_terms() {
    let index = make_index();
    let query = parse("artist:daft year:>1990 -love");
    let result = index.search(&query, &|_| 0u64, &[TrackId(1), TrackId(2), TrackId(3)]);
    assert_eq!(result, vec![TrackId(1)]);
}

#[test]
fn preserves_candidate_order() {
    let index = make_index();
    let query = parse("daft");
    let result = index.search(&query, &|_| 0u64, &[TrackId(3), TrackId(1), TrackId(2)]);
    assert_eq!(result, vec![TrackId(3), TrackId(1)]);
}

#[test]
fn free_function_delegates_to_index() {
    let index = make_index();
    let query = parse("punk");
    let candidates = vec![TrackId(1), TrackId(2), TrackId(3)];
    assert_eq!(
        search(&index, &query, &|_| 0u64, &candidates),
        index.search(&query, &|_| 0u64, &candidates)
    );
}
