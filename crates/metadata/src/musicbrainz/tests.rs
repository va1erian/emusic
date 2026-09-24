//! MusicBrainz provider tests: JSON parsing, query building and the
//! (opt-in) live-network smoke test.

use std::time::Duration;

use super::*;
use crate::Provider;

/// A representative WS/2 recording search response.
const FIXTURE: &str = include_str!("fixtures/recording_search.json");

fn parse_fixture() -> Vec<Candidate> {
    let response: parse::RecordingSearch = serde_json::from_str(FIXTURE).expect("fixture is valid");
    parse::to_candidates(&response)
}

#[test]
fn parses_the_common_fields_of_a_recording() {
    let candidates = parse_fixture();
    assert_eq!(candidates.len(), 2);

    let first = &candidates[0];
    assert_eq!(first.title.as_deref(), Some("Around the World"));
    assert_eq!(first.artist.as_deref(), Some("Daft Punk"));
    assert_eq!(first.album.as_deref(), Some("Homework"));
    assert_eq!(first.album_artist.as_deref(), Some("Daft Punk"));
    assert_eq!(first.year, Some(1997));
    assert_eq!(first.track_no, Some(5));
    assert_eq!(first.disc_no, Some(1));
    assert_eq!(first.duration, Some(Duration::from_millis(428_000)));
}

#[test]
fn prefers_an_official_release_over_a_later_bootleg() {
    let candidates = parse_fixture();
    let first = &candidates[0];
    assert_eq!(
        first.album.as_deref(),
        Some("Homework"),
        "the official 1997 release should win over the 2000 bootleg"
    );
    assert_eq!(first.album_artist.as_deref(), Some("Daft Punk"));
    assert_eq!(first.year, Some(1997));
    assert_eq!(first.disc_no, Some(1));
}

#[test]
fn joins_multiple_artist_credits() {
    let candidates = parse_fixture();
    assert_eq!(
        candidates[1].artist.as_deref(),
        Some("Daft Punk feat. Someone")
    );
}

#[test]
fn a_recording_without_a_release_has_no_album_fields() {
    let candidates = parse_fixture();
    let second = &candidates[1];
    assert_eq!(second.album, None);
    assert_eq!(second.album_artist, None);
    assert_eq!(second.year, None);
    assert_eq!(second.track_no, None);
    assert_eq!(second.disc_no, None);
}

#[test]
fn builds_a_lucene_query_from_the_present_fields() {
    let query = TrackQuery {
        title: Some("Around the World".to_string()),
        artist: Some("Daft Punk".to_string()),
        album: Some("Homework".to_string()),
        ..Default::default()
    };
    assert_eq!(
        lucene_query(&query),
        "artist:\"Daft Punk\" AND release:\"Homework\" AND recording:\"Around the World\""
    );
}

#[test]
fn escapes_lucene_special_characters() {
    let query = TrackQuery {
        artist: Some("AC/DC".to_string()),
        title: Some("Who made who?".to_string()),
        ..Default::default()
    };
    assert_eq!(
        lucene_query(&query),
        "artist:\"AC DC\" AND recording:\"Who made who\""
    );
}

#[test]
fn percent_encodes_reserved_characters() {
    assert_eq!(percent_encode("a b"), "a%20b");
    assert_eq!(percent_encode("a\"b&c"), "a%22b%26c");
    assert_eq!(percent_encode("A-z_0.9~"), "A-z_0.9~");
}

#[test]
fn derives_a_title_from_the_filename_when_the_tag_is_missing() {
    let query = TrackQuery {
        filename: Some("05 - Around the World.mp3".to_string()),
        ..Default::default()
    };
    let effective = effective_query(&query);
    assert_eq!(effective.title.as_deref(), Some("Around the World"));
}

#[test]
fn keeps_a_leading_number_that_is_part_of_the_title() {
    for (filename, expected) in [
        ("2001 A Space Odyssey.flac", "2001 A Space Odyssey"),
        ("7 Nation Army.mp3", "7 Nation Army"),
    ] {
        let query = TrackQuery {
            filename: Some(filename.to_string()),
            ..Default::default()
        };
        assert_eq!(
            effective_query(&query).title.as_deref(),
            Some(expected),
            "unexpected title for {filename}"
        );
    }
}

#[test]
fn does_not_treat_a_dot_in_the_title_as_an_extension() {
    let query = TrackQuery {
        filename: Some("Mr. Brightside.mp3".to_string()),
        ..Default::default()
    };
    assert_eq!(
        effective_query(&query).title.as_deref(),
        Some("Mr Brightside")
    );
}

#[test]
fn drops_a_term_that_escapes_to_nothing() {
    let query = TrackQuery {
        artist: Some("!!!".to_string()),
        title: Some("Song".to_string()),
        ..Default::default()
    };
    assert_eq!(lucene_query(&query), "recording:\"Song\"");
}

#[test]
fn keeps_the_title_tag_when_present() {
    let query = TrackQuery {
        title: Some("Tagged".to_string()),
        filename: Some("01 - Wrong.mp3".to_string()),
        ..Default::default()
    };
    assert_eq!(effective_query(&query).title.as_deref(), Some("Tagged"));
}

#[test]
fn a_query_with_no_usable_field_skips_the_network() {
    let provider = MusicBrainzProvider::new();
    let candidates = provider
        .search(&TrackQuery::default())
        .expect("an empty query is not an error");
    assert!(candidates.is_empty());
}

/// Live-network smoke test. Opt in with `EMUSIC_METADATA_NET_TESTS=1`; skipped
/// by default so CI never needs a network.
#[test]
fn live_search_is_opt_in() {
    if std::env::var("EMUSIC_METADATA_NET_TESTS").is_err() {
        return;
    }
    let provider = MusicBrainzProvider::new();
    let query = TrackQuery {
        artist: Some("Daft Punk".to_string()),
        title: Some("Around the World".to_string()),
        ..Default::default()
    };
    let candidates = provider.search(&query).expect("live search should succeed");
    assert!(
        !candidates.is_empty(),
        "expected at least one MusicBrainz match"
    );
}

/// A provider with canned results, to exercise the default trait methods.
struct StubProvider {
    results: Vec<Candidate>,
}

impl Provider for StubProvider {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn search(&self, _query: &TrackQuery) -> crate::Result<Vec<Candidate>> {
        Ok(self.results.clone())
    }
}

#[test]
fn best_match_returns_the_highest_scoring_candidate() {
    let provider = StubProvider {
        results: vec![
            Candidate {
                title: Some("Low".to_string()),
                score: 0.2,
                ..Default::default()
            },
            Candidate {
                title: Some("High".to_string()),
                score: 0.9,
                ..Default::default()
            },
        ],
    };
    let best = provider
        .best_match(&TrackQuery::default())
        .expect("a candidate exists");
    assert_eq!(best.title.as_deref(), Some("High"));
}

#[test]
fn best_match_without_candidates_is_no_match() {
    let provider = StubProvider {
        results: Vec::new(),
    };
    assert!(matches!(
        provider.best_match(&TrackQuery::default()),
        Err(MetadataError::NoMatch)
    ));
}
