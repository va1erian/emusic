//! The [MusicBrainz](https://musicbrainz.org) WS/2 provider.
//!
//! MusicBrainz is the open, file-oriented successor to freedb/gnudb (which are
//! keyed by a CD table of contents, not individual files). Its search API is
//! free, needs no key, and requires a meaningful `User-Agent` plus a client
//! average of at most one request per second.

mod parse;
#[cfg(test)]
mod tests;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::{Candidate, MetadataError, Provider, Result, TrackQuery};

/// The recording search endpoint.
const BASE_URL: &str = "https://musicbrainz.org/ws/2/recording";
/// MusicBrainz asks every client to average at most one request per second.
const MIN_INTERVAL: Duration = Duration::from_secs(1);
/// The most candidates to ask for and keep.
const LIMIT: usize = 10;
/// A whole-request timeout, so a stalled connection can never hang a worker.
const TIMEOUT: Duration = Duration::from_secs(15);
/// MusicBrainz requires an identifying `User-Agent` with a contact.
const USER_AGENT: &str = concat!(
    "emusic/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/va1erian/emusic)"
);

/// A [`Provider`] backed by the MusicBrainz web service.
///
/// Cheap to build; one instance is meant to be shared by a backend's auto-tag
/// worker so its rate-limit state and connection pool are reused.
#[derive(Debug)]
pub struct MusicBrainzProvider {
    agent: ureq::Agent,
    /// When the previous request was sent, for the one-request-per-second
    /// throttle. Holding the lock while sleeping serialises concurrent calls.
    last_request: Mutex<Option<Instant>>,
}

impl MusicBrainzProvider {
    /// Creates a provider with a bounded request timeout and our `User-Agent`.
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_global(Some(TIMEOUT))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
            last_request: Mutex::new(None),
        }
    }

    /// Sends the request, waiting first for the rate-limit budget.
    fn get(&self, url: &str) -> Result<String> {
        self.throttle();
        tracing::debug!(url, "querying MusicBrainz");
        match self.agent.get(url).call() {
            Ok(mut response) => response
                .body_mut()
                .read_to_string()
                .map_err(|error| MetadataError::Network(error.to_string())),
            Err(ureq::Error::StatusCode(503)) => Err(MetadataError::RateLimited),
            Err(ureq::Error::StatusCode(status)) => Err(MetadataError::Status { status }),
            Err(ureq::Error::HostNotFound | ureq::Error::ConnectionFailed) => {
                Err(MetadataError::Offline)
            }
            Err(error) => Err(MetadataError::Network(error.to_string())),
        }
    }

    /// Sleeps until at least [`MIN_INTERVAL`] has passed since the last request.
    fn throttle(&self) {
        let mut last_request = self
            .last_request
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(previous) = *last_request {
            let elapsed = previous.elapsed();
            if elapsed < MIN_INTERVAL {
                std::thread::sleep(MIN_INTERVAL - elapsed);
            }
        }
        *last_request = Some(Instant::now());
    }
}

impl Default for MusicBrainzProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for MusicBrainzProvider {
    fn name(&self) -> &'static str {
        "MusicBrainz"
    }

    fn search(&self, query: &TrackQuery) -> Result<Vec<Candidate>> {
        if !query.is_searchable() {
            return Ok(Vec::new());
        }
        let query = effective_query(query);
        let lucene = lucene_query(&query);
        if lucene.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!(
            "{BASE_URL}?query={}&fmt=json&limit={LIMIT}",
            percent_encode(&lucene)
        );
        let body = self.get(&url)?;
        let response: parse::RecordingSearch =
            serde_json::from_str(&body).map_err(|error| MetadataError::Parse(error.to_string()))?;

        let mut candidates = parse::to_candidates(&response);
        for candidate in &mut candidates {
            candidate.score = crate::score(&query, candidate);
        }
        candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
        candidates.truncate(LIMIT);
        Ok(candidates)
    }
}

/// Fills in a title from the filename when the file has no title tag, so a
/// lookup still has something to match on.
fn effective_query(query: &TrackQuery) -> TrackQuery {
    let mut query = query.clone();
    let has_title = query
        .title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty());
    if !has_title && let Some(filename) = query.filename.as_deref() {
        query.title = Some(title_from_filename(filename));
    }
    query
}

/// Turns a filename into a best-effort title: drop the extension, a leading
/// track number and common separators.
fn title_from_filename(filename: &str) -> String {
    let stem = strip_track_number(strip_extension(filename).trim());
    stem.replace(['_', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Drops a trailing file extension, but only a plausible one, so a dot in a
/// title (e.g. `"Mr. Brightside"`) isn't mistaken for an extension.
fn strip_extension(filename: &str) -> &str {
    match filename.rsplit_once('.') {
        Some((stem, extension))
            if !stem.is_empty()
                && (1..=5).contains(&extension.len())
                && extension.chars().all(|ch| ch.is_ascii_alphanumeric()) =>
        {
            stem
        }
        _ => filename,
    }
}

/// Strips a leading track number (`"01 - Title"`, `"1. Intro"`, `"2_Track"`),
/// but keeps a leading number that is part of the title (`"7 Nation Army"`,
/// `"2001 A Space Odyssey"`).
fn strip_track_number(stem: &str) -> &str {
    let digits = stem.len() - stem.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 || digits == stem.len() {
        return stem;
    }
    let rest = &stem[digits..];
    let looks_like_track_number = match rest.chars().next() {
        // A short number plus a non-space separator is almost always a track
        // number.
        Some('-' | '.' | '_') => digits <= 3,
        // A bare space is ambiguous; only zero-padded two-digit numbers are a
        // common convention.
        Some(' ') => digits == 2,
        _ => false,
    };
    if looks_like_track_number {
        rest.trim_start_matches([' ', '-', '_', '.'])
    } else {
        stem
    }
}

/// Builds a MusicBrainz Lucene query from whichever fields the track has.
fn lucene_query(query: &TrackQuery) -> String {
    [
        ("artist", query.artist.as_deref()),
        ("release", query.album.as_deref()),
        ("recording", query.title.as_deref()),
    ]
    .into_iter()
    .filter_map(|(field, value)| term(field, value))
    .collect::<Vec<_>>()
    .join(" AND ")
}

/// A `field:"value"` term, or `None` when the value is missing or escapes to
/// nothing — MusicBrainz would ignore an empty term, silently dropping the
/// constraint, so the term is omitted instead.
fn term(field: &str, value: Option<&str>) -> Option<String> {
    let escaped = escape(non_empty(value)?);
    (!escaped.is_empty()).then(|| format!("{field}:\"{escaped}\""))
}

/// `Some` only when the value has non-whitespace content.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.trim().is_empty())
}

/// Replaces Lucene's special characters with spaces so user text can't change
/// the query structure.
fn escape(term: &str) -> String {
    term.chars()
        .map(|ch| {
            if r#"\+-!(){}[]^"~*?:/&|"#.contains(ch) {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Percent-encodes everything but the RFC 3986 unreserved characters.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(char::from(byte));
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
