//! Reading tags and properties from streamed audio files with lofty.
//!
//! Only headers and metadata blocks are parsed, so scanning stays cheap even
//! for large libraries. Multi-value fields are joined with `"; "`, matching
//! the desktop scanner.

use std::path::Path;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{ItemKey, Tag};

/// Scalar metadata read from one streamed audio file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TagFields {
    /// Tagged title.
    pub title: Option<String>,
    /// Tagged artist.
    pub artist: Option<String>,
    /// Tagged album artist.
    pub album_artist: Option<String>,
    /// Tagged album.
    pub album: Option<String>,
    /// Tagged genre.
    pub genre: Option<String>,
    /// Tagged year.
    pub year: Option<i32>,
    /// Tagged track number.
    pub track_no: Option<u32>,
    /// Tagged disc number.
    pub disc_no: Option<u32>,
    /// Duration in seconds.
    pub duration_secs: Option<f64>,
    /// Channel count.
    pub channels: Option<u32>,
    /// Whether artwork is embedded in the file.
    pub has_embedded_art: bool,
}

/// Reads tags and stream properties from `path`.
///
/// # Errors
///
/// Returns the underlying lofty parse error when the file cannot be read.
pub fn read_stream(path: &Path) -> std::result::Result<TagFields, lofty::error::FileParseError> {
    let tagged_file = lofty::read_from_path(path)?;
    let properties = tagged_file.properties();
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let mut fields = TagFields {
        duration_secs: Some(properties.duration().as_secs_f64()),
        channels: properties.channels().map(u32::from),
        ..TagFields::default()
    };
    let duration = fields.duration_secs.unwrap_or(0.0);
    if !duration.is_finite() || duration <= 0.0 {
        fields.duration_secs = None;
    }

    if let Some(tag) = tag {
        fields.title = joined(tag, ItemKey::TrackTitle);
        fields.artist = joined(tag, ItemKey::TrackArtist);
        fields.album_artist = joined(tag, ItemKey::AlbumArtist);
        fields.album = joined(tag, ItemKey::AlbumTitle);
        fields.genre = joined(tag, ItemKey::Genre);
        fields.year = year_of(tag);
        fields.track_no = number_of(tag, ItemKey::TrackNumber);
        fields.disc_no = number_of(tag, ItemKey::DiscNumber);
        fields.has_embedded_art = !tag.pictures().is_empty();
    }
    Ok(fields)
}

/// All non-blank values of `key`, trimmed and joined with `"; "`.
fn joined(tag: &Tag, key: ItemKey) -> Option<String> {
    let joined = tag
        .get_strings(key)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    (!joined.is_empty()).then_some(joined)
}

fn year_of(tag: &Tag) -> Option<i32> {
    let raw = tag
        .get_strings(ItemKey::Year)
        .chain(tag.get_strings(ItemKey::RecordingDate))
        .find(|value| !value.trim().is_empty())?;
    leading_number(raw)
}

fn number_of(tag: &Tag, key: ItemKey) -> Option<u32> {
    leading_number(tag.get_strings(key).next()?).and_then(|number| u32::try_from(number).ok())
}

fn leading_number(raw: &str) -> Option<i32> {
    let digits: String = raw
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_file_fails_to_parse() {
        let path = std::env::temp_dir().join(format!(
            "emusic-srv-tags-{}-{}.flac",
            std::process::id(),
            crate::util::unix_now()
        ));
        std::fs::write(&path, b"not audio").unwrap();
        assert!(read_stream(&path).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_file_fails_to_parse() {
        assert!(read_stream(Path::new("does/not/exist.flac")).is_err());
    }
}
