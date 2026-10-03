//! Matches local library tracks to a server's tracks by metadata (#516).
//!
//! A local file has no server id, so its star can only be synced when its
//! tags identify exactly one server track: the same normalized artist (album
//! artist as a fallback), album and title, plus the same track number when
//! both sides have one. Normalization folds case and diacritics the way the
//! search engine does, trims, and collapses runs of whitespace.

use std::collections::HashMap;

use emusic_search::normalize_text;

/// The tags a track is matched on.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MatchFields<'a> {
    /// Title.
    pub title: Option<&'a str>,
    /// Artist.
    pub artist: Option<&'a str>,
    /// Album artist, used when the artist is missing.
    pub album_artist: Option<&'a str>,
    /// Album.
    pub album: Option<&'a str>,
    /// Track number.
    pub track_no: Option<u32>,
}

/// The normalized (artist, album, title) a track is grouped by.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MatchKey {
    artist: String,
    album: String,
    title: String,
}

impl MatchKey {
    /// The key of `fields`, or `None` when the title or artist is missing
    /// (too little to identify a track; an empty album is allowed).
    fn of(fields: &MatchFields<'_>) -> Option<Self> {
        let title = normalize(fields.title)?;
        let artist = normalize(fields.artist).or_else(|| normalize(fields.album_artist))?;
        let album = normalize(fields.album).unwrap_or_default();
        Some(Self {
            artist,
            album,
            title,
        })
    }
}

/// Folds case and diacritics, trims and collapses whitespace; `None` for a
/// missing or blank value.
fn normalize(value: Option<&str>) -> Option<String> {
    let folded = normalize_text(value?);
    let collapsed = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    (!collapsed.is_empty()).then_some(collapsed)
}

/// An index of one server's tracks by [`MatchKey`].
#[derive(Debug, Default)]
pub(crate) struct Matcher {
    by_key: HashMap<MatchKey, Vec<(String, Option<u32>)>>,
}

impl Matcher {
    /// Indexes the server tracks `(server id, fields)`. Tracks without a
    /// usable key are left out.
    pub(crate) fn new<'a>(tracks: impl IntoIterator<Item = (&'a str, MatchFields<'a>)>) -> Self {
        let mut by_key: HashMap<MatchKey, Vec<(String, Option<u32>)>> = HashMap::new();
        for (id, fields) in tracks {
            if let Some(key) = MatchKey::of(&fields) {
                by_key
                    .entry(key)
                    .or_default()
                    .push((id.to_string(), fields.track_no));
            }
        }
        Self { by_key }
    }

    /// The id of the single server track `local` matches, or `None` when
    /// there is no match or it is ambiguous.
    ///
    /// Candidates share the key; when both sides have a track number they
    /// must agree. A unique exact track-number match breaks a tie between
    /// several candidates (e.g. a candidate without a number and one with).
    pub(crate) fn find(&self, local: &MatchFields<'_>) -> Option<&str> {
        let candidates = self.by_key.get(&MatchKey::of(local)?)?;
        let Some(number) = local.track_no else {
            return single(candidates.iter());
        };
        let compatible = candidates
            .iter()
            .filter(|(_, other)| other.is_none_or(|other| other == number));
        if let Some(id) = single(compatible) {
            return Some(id);
        }
        single(
            candidates
                .iter()
                .filter(|(_, other)| *other == Some(number)),
        )
    }
}

/// The id when `candidates` holds exactly one entry.
fn single<'a>(mut candidates: impl Iterator<Item = &'a (String, Option<u32>)>) -> Option<&'a str> {
    let (id, _) = candidates.next()?;
    candidates.next().is_none().then_some(id.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields<'a>(
        artist: &'a str,
        album: &'a str,
        title: &'a str,
        no: Option<u32>,
    ) -> MatchFields<'a> {
        MatchFields {
            title: Some(title),
            artist: Some(artist),
            album_artist: None,
            album: Some(album),
            track_no: no,
        }
    }

    #[test]
    fn matches_across_case_accents_and_whitespace() {
        let matcher = Matcher::new([("s1", fields("Björk", "Début", "Human  Behaviour", Some(1)))]);
        let local = fields("  bjork ", "DEBUT", "human behaviour", Some(1));
        assert_eq!(matcher.find(&local), Some("s1"));
    }

    #[test]
    fn falls_back_to_the_album_artist() {
        let matcher = Matcher::new([("s1", fields("Artist", "Album", "Song", None))]);
        let local = MatchFields {
            artist: None,
            album_artist: Some("artist"),
            ..fields("", "Album", "Song", None)
        };
        assert_eq!(matcher.find(&local), Some("s1"));
        // A blank artist also falls back.
        let local = MatchFields {
            artist: Some("  "),
            album_artist: Some("Artist"),
            ..local
        };
        assert_eq!(matcher.find(&local), Some("s1"));
    }

    #[test]
    fn missing_title_or_artist_never_matches() {
        let matcher = Matcher::new([
            ("s1", fields("Artist", "Album", "Song", None)),
            (
                "s2",
                MatchFields {
                    title: None,
                    ..fields("Artist", "Album", "", None)
                },
            ),
        ]);
        let no_title = MatchFields {
            title: None,
            ..fields("Artist", "Album", "", None)
        };
        assert_eq!(matcher.find(&no_title), None);
        let no_artist = MatchFields {
            artist: None,
            ..fields("", "Album", "Song", None)
        };
        assert_eq!(matcher.find(&no_artist), None);
        // An album missing on both sides still matches.
        let matcher = Matcher::new([(
            "s3",
            MatchFields {
                album: None,
                ..fields("Artist", "", "Single", None)
            },
        )]);
        let local = MatchFields {
            album: Some(" "),
            ..fields("Artist", "", "Single", None)
        };
        assert_eq!(matcher.find(&local), Some("s3"));
    }

    #[test]
    fn different_tags_do_not_match() {
        let matcher = Matcher::new([("s1", fields("Artist", "Album", "Song", None))]);
        assert_eq!(matcher.find(&fields("Artist", "Other", "Song", None)), None);
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Song 2", None)),
            None
        );
    }

    #[test]
    fn ambiguous_matches_are_not_synced() {
        let matcher = Matcher::new([
            ("s1", fields("Artist", "Album", "Intro", None)),
            ("s2", fields("Artist", "Album", "Intro", None)),
        ]);
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Intro", None)),
            None
        );
        // A track number on the local side alone cannot tell them apart.
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Intro", Some(1))),
            None
        );
    }

    #[test]
    fn track_numbers_must_agree_when_both_are_present() {
        let matcher = Matcher::new([("s1", fields("Artist", "Album", "Song", Some(3)))]);
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Song", Some(4))),
            None
        );
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Song", Some(3))),
            Some("s1")
        );
        // A number on only one side is not compared.
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Song", None)),
            Some("s1")
        );
    }

    #[test]
    fn track_number_breaks_ties() {
        let matcher = Matcher::new([
            ("s1", fields("Artist", "Album", "Intro", Some(1))),
            ("s2", fields("Artist", "Album", "Intro", Some(9))),
            ("s3", fields("Artist", "Album", "Intro", None)),
        ]);
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Intro", Some(9))),
            Some("s2")
        );
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Intro", Some(5))),
            Some("s3")
        );
        assert_eq!(
            matcher.find(&fields("Artist", "Album", "Intro", None)),
            None
        );
    }
}
