//! The local metadata a lookup is seeded from.

use std::time::Duration;

/// The local metadata used to look a track up online.
///
/// Every field is optional: a lookup can be seeded from existing tags, the
/// filename, or both. Fields that are absent contribute nothing to the match
/// score (see [`score`](crate::score)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackQuery {
    /// Tagged title, when the file has one.
    pub title: Option<String>,
    /// Tagged artist, when the file has one.
    pub artist: Option<String>,
    /// Tagged album, when the file has one.
    pub album: Option<String>,
    /// The track's duration, used as a tie-breaker.
    pub duration: Option<Duration>,
    /// The file's name (with or without extension), used as a title fallback
    /// when the file has no title tag.
    pub filename: Option<String>,
}

impl TrackQuery {
    /// Whether the query has anything at all to search with.
    ///
    /// A query with only a duration has nothing a provider could match on, so
    /// callers can skip the round-trip when this is `false`.
    pub fn is_searchable(&self) -> bool {
        [&self.title, &self.artist, &self.album, &self.filename]
            .into_iter()
            .any(|field| {
                field
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_with_any_text_field_is_searchable() {
        let query = TrackQuery {
            filename: Some("song.mp3".to_string()),
            ..Default::default()
        };
        assert!(query.is_searchable());
    }

    #[test]
    fn a_query_with_only_a_duration_is_not_searchable() {
        let query = TrackQuery {
            duration: Some(Duration::from_secs(200)),
            ..Default::default()
        };
        assert!(!query.is_searchable());
    }

    #[test]
    fn blank_fields_do_not_count_as_searchable() {
        let query = TrackQuery {
            title: Some("   ".to_string()),
            artist: Some(String::new()),
            ..Default::default()
        };
        assert!(!query.is_searchable());
    }
}
