//! Reading tags and properties from streamed audio files with lofty.
//!
//! lofty parses headers and metadata blocks, not whole files, which keeps
//! incremental scans cheap even over SMB shares. Multi-value fields (e.g.
//! several `ARTIST` entries) are joined with `"; "`.

use std::time::Duration;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{ItemKey, Tag};

use emusic_core::{ArtSource, Track};

#[cfg(test)]
use emusic_core::TrackKind;

use super::walk::FoundFile;

/// Reads tags, properties and artwork presence for one streamed audio file.
///
/// The returned track has `kind` [`TrackKind::Stream`], `id` unassigned and
/// `added_at` set to `now`; the store fills in the rest.
pub(crate) fn read_stream(
    file: &FoundFile,
    now: i64,
) -> Result<Track, lofty::error::FileParseError> {
    let tagged_file = lofty::read_from_path(&file.path)?;
    let properties = tagged_file.properties();
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let mut track = super::untagged_track(file, now);
    track.duration_ms = duration_ms(properties.duration());
    track.bitrate = properties.audio_bitrate().or(properties.overall_bitrate());
    track.sample_rate = properties.sample_rate();
    track.channels = properties.channels();

    if let Some(tag) = tag {
        track.title = joined(tag, ItemKey::TrackTitle);
        track.artist = joined(tag, ItemKey::TrackArtist);
        track.album_artist = joined(tag, ItemKey::AlbumArtist);
        track.album = joined(tag, ItemKey::AlbumTitle);
        track.genre = joined(tag, ItemKey::Genre);
        track.year = year_of(tag);
        track.track_no = number_of(tag, ItemKey::TrackNumber);
        track.disc_no = number_of(tag, ItemKey::DiscNumber);
        track.composer = joined(tag, ItemKey::Composer);
        track.comment = joined(tag, ItemKey::Comment);
        if !tag.pictures().is_empty() {
            track.art_source = ArtSource::Embedded;
        }
    }
    Ok(track)
}

/// All non-blank text values of `key`, trimmed and joined with `"; "`.
fn joined(tag: &Tag, key: ItemKey) -> Option<String> {
    let joined = tag
        .get_strings(key)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    (!joined.is_empty()).then_some(joined)
}

/// The release year, accepting both plain years (`1999`) and full dates
/// (`2021-05-03`), from the `Year` or `Date` fields.
fn year_of(tag: &Tag) -> Option<i32> {
    let raw = tag
        .get_strings(ItemKey::Year)
        .chain(tag.get_strings(ItemKey::RecordingDate))
        .find(|value| !value.trim().is_empty())?;
    leading_number(raw)
}

/// A track/disc number, accepting `NN` and `NN/TT` forms.
fn number_of(tag: &Tag, key: ItemKey) -> Option<u32> {
    leading_number(tag.get_strings(key).next()?).and_then(|number| u32::try_from(number).ok())
}

/// Parses the leading ASCII digits of `raw` as a number.
fn leading_number(raw: &str) -> Option<i32> {
    let digits: String = raw
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// A duration in milliseconds, clamped to what a `u32` can hold.
fn duration_ms(duration: Duration) -> u32 {
    duration.as_millis().min(u32::MAX as u128) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn found(path: &Path) -> FoundFile {
        std::fs::write(path, b"not a real audio file").unwrap();
        FoundFile {
            path: path.to_path_buf(),
            size: 23,
            mtime: 1_700_000_000,
            kind: TrackKind::Stream,
        }
    }

    #[test]
    fn leading_number_parses_plain_and_slash_suffixed_forms() {
        assert_eq!(leading_number("12"), Some(12));
        assert_eq!(leading_number(" 3/12 "), Some(3));
        assert_eq!(leading_number("1999-05-03"), Some(1999));
        assert_eq!(leading_number(""), None);
        assert_eq!(leading_number("n/a"), None);
    }

    #[test]
    fn duration_ms_clamps_huge_durations() {
        let century = Duration::from_secs(100 * 365 * 24 * 3600);
        assert_eq!(duration_ms(Duration::from_millis(1234)), 1234);
        assert_eq!(duration_ms(century), u32::MAX);
    }

    #[test]
    fn garbage_file_fails_to_parse() {
        let dir = std::env::temp_dir().join(format!("emusic-tags-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("broken.flac");
        let file = found(&path);
        assert!(read_stream(&file, 1_700_000_000).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_fails_to_parse() {
        let file = FoundFile {
            path: PathBuf::from("this/file/does/not/exist.flac"),
            size: 0,
            mtime: 0,
            kind: TrackKind::Stream,
        };
        assert!(read_stream(&file, 0).is_err());
    }
}
