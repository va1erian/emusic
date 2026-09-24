//! Parsing a MusicBrainz WS/2 recording search response into [`Candidate`]s.
//!
//! Only the fields the tag editor can write are kept; everything else in the
//! response is ignored. The structs are `pub(crate)` so the module's tests can
//! build them from the checked-in JSON fixture.

use std::time::Duration;

use serde::Deserialize;

use crate::Candidate;

/// The top level of `/ws/2/recording?...&fmt=json`.
#[derive(Debug, Deserialize)]
pub(crate) struct RecordingSearch {
    #[serde(default)]
    pub(crate) recordings: Vec<Recording>,
}

/// One recording in the response.
#[derive(Debug, Deserialize)]
pub(crate) struct Recording {
    pub(crate) title: Option<String>,
    /// Length in milliseconds.
    pub(crate) length: Option<u64>,
    /// The date of the recording's earliest release, the most reliable year
    /// source when a recording appears on many releases.
    #[serde(rename = "first-release-date")]
    pub(crate) first_release_date: Option<String>,
    #[serde(rename = "artist-credit", default)]
    pub(crate) artist_credit: Vec<ArtistCredit>,
    #[serde(default)]
    pub(crate) releases: Vec<Release>,
}

/// An artist credit: a credited name plus how it joins the next one.
#[derive(Debug, Deserialize)]
pub(crate) struct ArtistCredit {
    pub(crate) name: Option<String>,
    pub(crate) joinphrase: Option<String>,
    pub(crate) artist: Option<Artist>,
}

/// The canonical artist behind a credit.
#[derive(Debug, Deserialize)]
pub(crate) struct Artist {
    pub(crate) name: Option<String>,
}

/// A release (album) a recording appears on.
#[derive(Debug, Deserialize)]
pub(crate) struct Release {
    pub(crate) title: Option<String>,
    pub(crate) date: Option<String>,
    /// Release status, e.g. `"Official"` or `"Bootleg"`.
    pub(crate) status: Option<String>,
    #[serde(rename = "artist-credit", default)]
    pub(crate) artist_credit: Vec<ArtistCredit>,
    #[serde(default)]
    pub(crate) media: Vec<Medium>,
}

/// A disc of a release.
#[derive(Debug, Deserialize)]
pub(crate) struct Medium {
    pub(crate) position: Option<u32>,
    #[serde(default)]
    pub(crate) track: Vec<Track>,
}

/// The position of the recording on a medium.
#[derive(Debug, Deserialize)]
pub(crate) struct Track {
    pub(crate) number: Option<String>,
}

/// Maps every recording in `search` to a candidate.
pub(crate) fn to_candidates(search: &RecordingSearch) -> Vec<Candidate> {
    search
        .recordings
        .iter()
        .map(recording_to_candidate)
        .collect()
}

/// Maps one recording, taking the album fields from its selected release.
fn recording_to_candidate(recording: &Recording) -> Candidate {
    let release = select_release(recording);
    let (disc_no, track_no) = release.map(track_position).unwrap_or((None, None));
    let year = recording
        .first_release_date
        .as_deref()
        .and_then(parse_year)
        .or_else(|| {
            release
                .and_then(|release| release.date.as_deref())
                .and_then(parse_year)
        });

    Candidate {
        title: recording.title.clone(),
        artist: join_credit(&recording.artist_credit),
        album: release.and_then(|release| release.title.clone()),
        album_artist: release.and_then(|release| join_credit(&release.artist_credit)),
        year,
        track_no,
        disc_no,
        duration: recording.length.map(Duration::from_millis),
        ..Default::default()
    }
}

/// Picks the release to take the album fields from.
///
/// A recording often appears on many releases, and the search result's order is
/// not meaningful, so an official release wins over a bootleg and, among those,
/// the earliest date wins — otherwise a later compilation could supply the
/// album and its artist. A release with no date sorts last.
fn select_release(recording: &Recording) -> Option<&Release> {
    recording
        .releases
        .iter()
        .filter(|release| release.title.is_some())
        .min_by_key(|release| {
            let official = u8::from(release.status.as_deref() != Some("Official"));
            (official, release.date.as_deref().unwrap_or("\u{10FFFF}"))
        })
}

/// The disc and track numbers of the first track on the first medium.
fn track_position(release: &Release) -> (Option<u32>, Option<u32>) {
    let Some(medium) = release.media.first() else {
        return (None, None);
    };
    let track_no = medium
        .track
        .first()
        .and_then(|track| track.number.as_deref())
        .and_then(parse_number);
    (medium.position, track_no)
}

/// Joins an artist credit list into a single display name.
///
/// A credit's join phrase is only appended after an actual name, so a credit
/// with a missing name can't leave a dangling `" feat. "` in the result.
fn join_credit(credits: &[ArtistCredit]) -> Option<String> {
    let mut name = String::new();
    for credit in credits {
        let Some(credited) = credit.name.as_deref().or_else(|| {
            credit
                .artist
                .as_ref()
                .and_then(|artist| artist.name.as_deref())
        }) else {
            continue;
        };
        name.push_str(credited);
        if let Some(joinphrase) = &credit.joinphrase {
            name.push_str(joinphrase);
        }
    }
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// Parses the leading four-digit year out of a MusicBrainz date.
fn parse_year(date: &str) -> Option<i32> {
    let digits: String = date.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() == 4 {
        digits.parse().ok()
    } else {
        None
    }
}

/// Parses the first number in a track number like `"3"` or `"A1"` (a vinyl
/// side letter followed by the track).
fn parse_number(number: &str) -> Option<u32> {
    let digits: String = number
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credit(name: Option<&str>, joinphrase: Option<&str>) -> ArtistCredit {
        ArtistCredit {
            name: name.map(str::to_string),
            joinphrase: joinphrase.map(str::to_string),
            artist: None,
        }
    }

    #[test]
    fn join_credit_skips_a_dangling_joinphrase() {
        let credits = [credit(None, Some(" feat. ")), credit(Some("Someone"), None)];
        assert_eq!(join_credit(&credits).as_deref(), Some("Someone"));
    }

    #[test]
    fn join_credit_falls_back_to_the_artist_name() {
        let credits = [ArtistCredit {
            name: None,
            joinphrase: None,
            artist: Some(Artist {
                name: Some("Fallback".to_string()),
            }),
        }];
        assert_eq!(join_credit(&credits).as_deref(), Some("Fallback"));
    }

    #[test]
    fn join_credit_of_nothing_is_none() {
        assert_eq!(join_credit(&[]), None);
    }

    #[test]
    fn parse_year_accepts_full_and_partial_dates() {
        assert_eq!(parse_year("1997"), Some(1997));
        assert_eq!(parse_year("1997-01-20"), Some(1997));
        assert_eq!(parse_year("05-1997"), None);
        assert_eq!(parse_year(""), None);
    }

    #[test]
    fn parse_number_accepts_a_vinyl_side_letter() {
        assert_eq!(parse_number("3"), Some(3));
        assert_eq!(parse_number("A1"), Some(1));
        assert_eq!(parse_number("A"), None);
    }
}
