//! One possible match returned by a provider.

use std::time::Duration;

/// A candidate tag set for a track.
///
/// The fields mirror the editable tags a frontend can write (plus the album
/// artist used for grouping); a frontend maps them onto its own tag form.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Candidate {
    /// Track title.
    pub title: Option<String>,
    /// Track artist.
    pub artist: Option<String>,
    /// Release (album) title.
    pub album: Option<String>,
    /// Release artist, used for compilations.
    pub album_artist: Option<String>,
    /// Genre, when the provider exposes one.
    pub genre: Option<String>,
    /// Release year.
    pub year: Option<i32>,
    /// Track number within its disc.
    pub track_no: Option<u32>,
    /// Disc number within the release.
    pub disc_no: Option<u32>,
    /// Composer, when the provider exposes one.
    pub composer: Option<String>,
    /// The recording's length, used as a tie-breaker when scoring.
    pub duration: Option<Duration>,
    /// Match confidence in `0.0..=1.0`, set by the provider that produced the
    /// candidate (see [`score`](crate::score)). Higher is a better match.
    pub score: f32,
}
