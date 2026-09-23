//! Reading the scalar tag fields the tag editor exposes.

use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::tag::{ItemKey, Tag};

use crate::error::{LibraryError, Result};

use super::EditableTags;

/// Reads the editable scalar tag fields of the audio file at `path`.
///
/// Uses the file's primary tag (falling back to its first tag, matching the
/// scanner). A file with no tags at all yields [`EditableTags::default`].
///
/// # Errors
///
/// Returns [`LibraryError::ReadTags`] if the file cannot be read or parsed.
pub fn read_tags(path: impl AsRef<Path>) -> Result<EditableTags> {
    let path = path.as_ref();
    let tagged_file = lofty::read_from_path(path).map_err(|source| LibraryError::ReadTags {
        path: path.to_path_buf(),
        source,
    })?;
    let Some(tag) = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())
    else {
        return Ok(EditableTags::default());
    };

    Ok(EditableTags {
        title: joined(tag, ItemKey::TrackTitle),
        artist: joined(tag, ItemKey::TrackArtist),
        album: joined(tag, ItemKey::AlbumTitle),
        album_artist: joined(tag, ItemKey::AlbumArtist),
        genre: joined(tag, ItemKey::Genre),
        year: year_of(tag),
        track_no: number_of(tag, ItemKey::TrackNumber),
        disc_no: number_of(tag, ItemKey::DiscNumber),
        composer: joined(tag, ItemKey::Composer),
        comment: joined(tag, ItemKey::Comment),
    })
}

/// All non-blank text values of `key`, trimmed and joined with `"; "`.
pub(crate) fn joined(tag: &Tag, key: ItemKey) -> Option<String> {
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
pub(crate) fn year_of(tag: &Tag) -> Option<i32> {
    let raw = tag
        .get_strings(ItemKey::Year)
        .chain(tag.get_strings(ItemKey::RecordingDate))
        .find(|value| !value.trim().is_empty())?;
    leading_number(raw)
}

/// A track/disc number, accepting `NN` and `NN/TT` forms.
pub(crate) fn number_of(tag: &Tag, key: ItemKey) -> Option<u32> {
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
