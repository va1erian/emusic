//! In-memory search index and matcher.
//!
//! The matcher turns a parsed [`Query`](crate::query::Query) into a list of
//! matching [`TrackId`]s. Text terms are evaluated with `memchr::memmem`
//! finders built once per query; numeric terms are evaluated against track
//! fields and a user-supplied [`PlayStats`] source for play counts.

use std::collections::HashMap;

use emusic_core::{Track, TrackId};

use crate::query::{Field, FieldValue, NumericSpec, Query, TermBody};

use self::haystack::Haystack;

pub mod haystack;
pub mod numeric;

/// External play-count stats required for `plays:` filters.
///
/// This small trait keeps the search crate from depending on the library
/// crate; callers can supply a closure, a hash map, or any other store.
pub trait PlayStats {
    /// Returns the play count for a track, defaulting to zero when unknown.
    fn play_count(&self, id: TrackId) -> u64;
}

impl<F> PlayStats for F
where
    F: Fn(TrackId) -> u64,
{
    fn play_count(&self, id: TrackId) -> u64 {
        self(id)
    }
}

/// An in-memory index of tracks ready for substring search.
#[derive(Debug, Clone, Default)]
pub struct Index {
    entries: HashMap<TrackId, Entry>,
}

#[derive(Debug, Clone)]
struct Entry {
    track: Track,
    haystack: Haystack,
}

impl Index {
    /// Creates an empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Adds a track to the index, replacing any existing entry with the same id.
    pub fn add_track(&mut self, track: Track) {
        let haystack = Haystack::build(&track);
        self.entries.insert(track.id, Entry { track, haystack });
    }

    /// Returns the number of tracks in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the index contains no tracks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Searches `candidates` and returns the ids that satisfy every query term,
    /// preserving the input order.
    ///
    /// Tracks whose id is not present in the index are silently skipped. For
    /// repeated searches with the same query, use [`PreparedQuery`] to avoid
    /// rebuilding the per-term finders.
    pub fn search<S: PlayStats>(
        &self,
        query: &Query,
        stats: &S,
        candidates: &[TrackId],
    ) -> Vec<TrackId> {
        self.search_prepared(&PreparedQuery::new(query), stats, candidates)
    }

    /// Searches using a query whose finders have already been built.
    pub fn search_prepared<S: PlayStats>(
        &self,
        query: &PreparedQuery<'_>,
        stats: &S,
        candidates: &[TrackId],
    ) -> Vec<TrackId> {
        if query.terms.is_empty() {
            return candidates.to_vec();
        }

        let mut results = Vec::new();

        for &id in candidates {
            let Some(entry) = self.entries.get(&id) else {
                continue;
            };
            if track_matches(&query.terms, entry, id, stats) {
                results.push(id);
            }
        }

        results
    }
}

/// Convenience free function that delegates to [`Index::search`].
pub fn search<S: PlayStats>(
    index: &Index,
    query: &Query,
    stats: &S,
    candidates: &[TrackId],
) -> Vec<TrackId> {
    index.search(query, stats, candidates)
}

/// A query with pre-built `memmem` finders, reused across multiple searches.
///
/// Building a [`PreparedQuery`] once and calling [`Index::search_prepared`]
/// avoids the per-search overhead of constructing finders.
#[derive(Debug)]
pub struct PreparedQuery<'q> {
    terms: Vec<PreparedTerm<'q>>,
}

impl<'q> PreparedQuery<'q> {
    /// Prepares a parsed query for repeated matching.
    #[must_use]
    pub fn new(query: &'q Query) -> Self {
        let mut terms: Vec<_> = query
            .terms
            .iter()
            .map(|term| match &term.body {
                TermBody::Text(text) => PreparedTerm::Text {
                    needle: text.text(),
                    field: None,
                    negated: term.negated,
                },
                TermBody::Field {
                    field,
                    value: FieldValue::Text(text),
                } => PreparedTerm::Text {
                    needle: text.text(),
                    field: Some(*field),
                    negated: term.negated,
                },
                TermBody::Field {
                    field,
                    value: FieldValue::Numeric(spec),
                } => PreparedTerm::Numeric {
                    spec: *spec,
                    field: *field,
                    negated: term.negated,
                },
            })
            .collect();
        // Evaluate cheap numeric and field-scoped filters first so the more
        // expensive full-haystack scan is skipped for non-matching tracks.
        terms.sort_by_key(|t| t.cost());
        Self { terms }
    }
}

#[derive(Debug)]
enum PreparedTerm<'q> {
    Text {
        needle: &'q str,
        field: Option<Field>,
        negated: bool,
    },
    Numeric {
        spec: NumericSpec,
        field: Field,
        negated: bool,
    },
}

impl<'q> PreparedTerm<'q> {
    /// Relative evaluation cost: lower is cheaper.
    fn cost(&self) -> u8 {
        match self {
            Self::Numeric { .. } => 0,
            Self::Text { field: Some(_), .. } => 1,
            Self::Text { field: None, .. } => 2,
        }
    }
}

fn track_matches<S: PlayStats>(
    prepared: &[PreparedTerm<'_>],
    entry: &Entry,
    id: TrackId,
    stats: &S,
) -> bool {
    for term in prepared {
        let (matched, negated) = match term {
            PreparedTerm::Text {
                needle,
                field: None,
                negated,
            } => (
                memchr::memmem::find(entry.haystack.full().as_bytes(), needle.as_bytes()).is_some(),
                *negated,
            ),
            PreparedTerm::Text {
                needle,
                field: Some(field),
                negated,
            } => (
                memchr::memmem::find(entry.haystack.field(*field).as_bytes(), needle.as_bytes())
                    .is_some(),
                *negated,
            ),
            PreparedTerm::Numeric {
                spec,
                field,
                negated,
            } => {
                let value = numeric_value(*field, &entry.track, id, stats);
                (numeric::matches(*spec, value), *negated)
            }
        };

        if matched == negated {
            return false;
        }
    }
    true
}

fn numeric_value<S: PlayStats>(field: Field, track: &Track, id: TrackId, stats: &S) -> f64 {
    match field {
        Field::Plays => stats.play_count(id) as f64,
        _ => numeric::track_value(field, track).unwrap_or(f64::NAN),
    }
}

#[cfg(test)]
mod tests;
