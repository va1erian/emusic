//! Ranking a candidate against the local metadata a lookup started from.
//!
//! The score is a weighted average of per-field similarities, renormalised over
//! the fields the query actually has. It is a pure function so it can be tested
//! without a network and reused by any [`Provider`](crate::Provider).

use std::collections::HashSet;
use std::time::Duration;

use crate::{Candidate, TrackQuery};

/// Title similarity weight — the strongest signal.
const W_TITLE: f32 = 0.5;
/// Artist similarity weight.
const W_ARTIST: f32 = 0.3;
/// Album similarity weight.
const W_ALBUM: f32 = 0.1;
/// Duration closeness weight.
const W_DURATION: f32 = 0.1;

/// Scores a candidate against `query`, in `0.0..=1.0`.
///
/// Only fields present on both sides contribute; the weights of the missing
/// ones are redistributed. A query and candidate that share no fields score
/// `0.0`.
pub fn score(query: &TrackQuery, candidate: &Candidate) -> f32 {
    let mut weighted = 0.0;
    let mut total = 0.0;

    if let (Some(query_title), Some(candidate_title)) = (
        non_empty(query.title.as_deref()),
        non_empty(candidate.title.as_deref()),
    ) {
        weighted += W_TITLE * similarity(query_title, candidate_title);
        total += W_TITLE;
    }
    if let (Some(query_artist), Some(candidate_artist)) = (
        non_empty(query.artist.as_deref()),
        non_empty(candidate.artist.as_deref()),
    ) {
        weighted += W_ARTIST * similarity(query_artist, candidate_artist);
        total += W_ARTIST;
    }
    if let (Some(query_album), Some(candidate_album)) = (
        non_empty(query.album.as_deref()),
        non_empty(candidate.album.as_deref()),
    ) {
        weighted += W_ALBUM * similarity(query_album, candidate_album);
        total += W_ALBUM;
    }
    if let (Some(query_duration), Some(candidate_duration)) = (query.duration, candidate.duration) {
        weighted += W_DURATION * duration_similarity(query_duration, candidate_duration);
        total += W_DURATION;
    }

    if total > 0.0 { weighted / total } else { 0.0 }
}

/// `Some` only when the value has non-whitespace content.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.trim().is_empty())
}

/// Token-set similarity in `0.0..=1.0`: exact match, then Jaccard overlap, with
/// a boost when the shorter token set is fully contained in the longer.
fn similarity(a: &str, b: &str) -> f32 {
    let a = normalize(a);
    let b = normalize(b);
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }

    let a_tokens = tokens(&a);
    let b_tokens = tokens(&b);
    let shared = a_tokens.intersection(&b_tokens).count() as f32;
    let union = a_tokens.union(&b_tokens).count() as f32;
    let jaccard = if union > 0.0 { shared / union } else { 0.0 };

    let (shorter, longer) = if a_tokens.len() <= b_tokens.len() {
        (&a_tokens, &b_tokens)
    } else {
        (&b_tokens, &a_tokens)
    };
    if !shorter.is_empty() && shorter.is_subset(longer) {
        jaccard.max(0.9)
    } else {
        jaccard
    }
}

/// Duration closeness: full credit within two seconds, partial within ten.
fn duration_similarity(query: Duration, candidate: Duration) -> f32 {
    let diff = query.abs_diff(candidate).as_secs_f32();
    if diff <= 2.0 {
        1.0
    } else if diff <= 10.0 {
        0.5
    } else {
        0.0
    }
}

/// Lowercases and reduces to space-separated alphanumeric tokens.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.extend(ch.to_lowercase());
        } else {
            pending_space = true;
        }
    }
    out
}

/// The non-empty space-separated tokens of an already-normalized string.
fn tokens(normalized: &str) -> HashSet<&str> {
    normalized
        .split(' ')
        .filter(|token| !token.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(title: &str, artist: &str) -> Candidate {
        Candidate {
            title: Some(title.to_string()),
            artist: Some(artist.to_string()),
            ..Default::default()
        }
    }

    fn query(title: &str, artist: &str) -> TrackQuery {
        TrackQuery {
            title: Some(title.to_string()),
            artist: Some(artist.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn exact_title_and_artist_scores_one() {
        let score = score(
            &query("Daft Punk", "Harder Better"),
            &candidate("Daft Punk", "Harder Better"),
        );
        assert!((score - 1.0).abs() < 1e-6, "expected 1.0, got {score}");
    }

    #[test]
    fn a_wrong_artist_lowers_the_score() {
        let exact = score(&query("Song", "Artist"), &candidate("Song", "Artist"));
        let wrong = score(&query("Song", "Artist"), &candidate("Song", "Someone Else"));
        assert!(wrong < exact, "{wrong} should be below {exact}");
    }

    #[test]
    fn casing_and_punctuation_are_ignored() {
        let score = score(
            &query("don't stop me now", "Queen"),
            &candidate("Don't Stop Me Now", "QUEEN"),
        );
        assert!((score - 1.0).abs() < 1e-6, "expected 1.0, got {score}");
    }

    #[test]
    fn a_contained_title_gets_a_boost() {
        let score = score(
            &query("Harder Better Faster Stronger", "Daft Punk"),
            &candidate("Harder Better Faster Stronger (Remastered)", "Daft Punk"),
        );
        assert!(score > 0.9, "expected a boosted score, got {score}");
    }

    #[test]
    fn duration_is_a_tie_breaker() {
        let close = Candidate {
            duration: Some(Duration::from_secs(200)),
            ..candidate("Song", "Artist")
        };
        let far = Candidate {
            duration: Some(Duration::from_secs(400)),
            ..candidate("Song", "Artist")
        };
        let mut q = query("Song", "Artist");
        q.duration = Some(Duration::from_secs(201));
        assert!(score(&q, &close) > score(&q, &far));
    }

    #[test]
    fn missing_query_fields_are_ignored() {
        let mut q = query("Song", "Artist");
        q.album = None;
        let with_album = Candidate {
            album: Some("Unrelated Album".to_string()),
            ..candidate("Song", "Artist")
        };
        assert!((score(&q, &with_album) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn nothing_in_common_scores_zero() {
        let score = score(&query("Song", "Artist"), &candidate("", ""));
        assert_eq!(score, 0.0);
    }
}
