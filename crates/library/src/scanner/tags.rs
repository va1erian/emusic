//! Reading tags and properties from streamed audio files with lofty.
//!
//! lofty parses headers and metadata blocks, not whole files, which keeps
//! incremental scans cheap even over SMB shares. Multi-value fields (e.g.
//! several `ARTIST` entries) are joined with `"; "`.

use std::time::Duration;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::ItemKey;

use emusic_core::{ArtSource, Track};

#[cfg(test)]
use emusic_core::TrackKind;

use crate::tags::read::{joined, number_of, year_of};

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
