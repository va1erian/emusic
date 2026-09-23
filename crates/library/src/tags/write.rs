//! Writing the scalar tag fields the tag editor exposes back to a file.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::tag::{ItemKey, Tag, TagExt};

use crate::error::{LibraryError, Result};

use super::EditableTags;

/// Writes the editable scalar fields of `tags` to the audio file at `path`.
///
/// The file's existing primary tag (or, failing that, its first tag, matching
/// [`read_tags`](super::read_tags)) is loaded first and only the exposed
/// scalar fields are changed, so embedded artwork and unexposed tag items are
/// preserved. A field set to `None` (or blank text) is removed.
///
/// # Errors
///
/// Returns [`LibraryError::ReadTags`] if the file cannot be read, or
/// [`LibraryError::WriteTags`] if the updated tag cannot be saved.
pub fn write_tags(path: impl AsRef<Path>, tags: &EditableTags) -> Result<()> {
    let path = path.as_ref();
    let tagged_file = lofty::read_from_path(path).map_err(|source| LibraryError::ReadTags {
        path: path.to_path_buf(),
        source,
    })?;

    let mut tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())
        .cloned()
        .unwrap_or_else(|| Tag::new(tagged_file.primary_tag_type()));

    apply(&mut tag, tags);

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|source| LibraryError::WriteTags {
            path: path.to_path_buf(),
            source,
        })
}

/// Applies every scalar field to `tag`, removing the items set to `None`.
fn apply(tag: &mut Tag, tags: &EditableTags) {
    set_text(tag, ItemKey::TrackTitle, tags.title.as_deref());
    set_text(tag, ItemKey::TrackArtist, tags.artist.as_deref());
    set_text(tag, ItemKey::AlbumTitle, tags.album.as_deref());
    set_text(tag, ItemKey::AlbumArtist, tags.album_artist.as_deref());
    set_text(tag, ItemKey::Genre, tags.genre.as_deref());
    set_text(tag, ItemKey::Composer, tags.composer.as_deref());
    set_text(tag, ItemKey::Comment, tags.comment.as_deref());
    // The scanner reads years from `Year`/`Date`; `RecordingDate` is the key
    // every format maps a year or date onto.
    set_text(
        tag,
        ItemKey::RecordingDate,
        tags.year.map(|year| year.to_string()).as_deref(),
    );
    set_text(
        tag,
        ItemKey::TrackNumber,
        tags.track_no.map(|number| number.to_string()).as_deref(),
    );
    set_text(
        tag,
        ItemKey::DiscNumber,
        tags.disc_no.map(|number| number.to_string()).as_deref(),
    );
}

/// Sets `key` to the trimmed `value`, or removes it when blank/absent.
fn set_text(tag: &mut Tag, key: ItemKey, value: Option<&str>) {
    match value.map(str::trim) {
        Some(text) if !text.is_empty() => {
            tag.insert_text(key, text.to_string());
        }
        _ => tag.remove_key(key),
    }
}
