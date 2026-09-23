//! Reading and writing the editable scalar tag fields of audio files.
//!
//! [`read_tags`] and [`write_tags`] work directly on a file path — no
//! [`Store`](crate::Store) or UI involved — using `lofty`. [`EditableTags`] is
//! a plain snapshot of the fields the tag editor exposes; every field is
//! optional so an absent field round-trips as `None` rather than an empty
//! string.
//!
//! Writes load the file's existing primary tag, change only the fields set on
//! [`EditableTags`] and save it back, so embedded artwork and any tag items
//! the editor does not expose are preserved.

pub(crate) mod edit;
pub(crate) mod read;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod write;

pub use edit::{EditOutcome, EditRequest, edit_tags};
pub use read::read_tags;
pub use write::write_tags;

/// The scalar tag fields the tag editor can change.
///
/// Each field mirrors its [`Track`](emusic_core::Track) counterpart: `None`
/// means the field is absent from the file, and writing `None` removes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditableTags {
    /// Tagged title.
    pub title: Option<String>,
    /// Tagged track artist.
    pub artist: Option<String>,
    /// Tagged album name.
    pub album: Option<String>,
    /// Tagged album artist, used for grouping compilations.
    pub album_artist: Option<String>,
    /// Tagged genre.
    pub genre: Option<String>,
    /// Tagged release year.
    pub year: Option<i32>,
    /// Tagged track number within the album.
    pub track_no: Option<u32>,
    /// Tagged disc number within the release.
    pub disc_no: Option<u32>,
    /// Tagged composer.
    pub composer: Option<String>,
    /// Free-form tagged comment.
    pub comment: Option<String>,
}
