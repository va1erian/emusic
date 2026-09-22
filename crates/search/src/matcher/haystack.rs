//! Folded haystack construction for fast substring matching.
//!
//! Each track is flattened into a single lower-cased, diacritic-folded string
//! with a unit separator (`\x1f`) between fields. Per-field byte offsets are
//! stored so field-scoped filters can search only the relevant slice.

use std::ops::Range;

use crate::query::Field;
use crate::query::normalize::normalize_text;

/// The unit separator used between folded fields in the haystack.
pub const FIELD_SEP: char = '\x1f';

/// A pre-computed haystack for one track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Haystack {
    text: String,
    offsets: FieldOffsets,
}

impl Haystack {
    /// Builds a haystack from a track.
    #[must_use]
    pub fn build(track: &emusic_core::Track) -> Self {
        let mut builder = Builder::new();

        let artist = builder.push(track.artist.as_deref());
        let album = builder.push(track.album.as_deref());
        let album_artist = builder.push(track.effective_album_artist());
        let title = builder.push(track.title.as_deref());
        let genre = builder.push(track.genre.as_deref());
        let file = builder.push(Some(&track.filename));
        let dir = {
            let text = track.dir.to_string_lossy();
            builder.push(Some(&text))
        };
        let ext = builder.push(Some(&track.ext));
        let comment = builder.push(track.comment.as_deref());
        let composer = builder.push(track.composer.as_deref());

        Self {
            text: builder.text,
            offsets: FieldOffsets {
                artist,
                album,
                album_artist,
                title,
                genre,
                file,
                dir,
                ext,
                comment,
                composer,
            },
        }
    }

    /// The full folded text, searchable for unscoped terms.
    #[must_use]
    pub fn full(&self) -> &str {
        &self.text
    }

    /// The folded text for a single field, if that field is textual.
    #[must_use]
    pub fn field(&self, field: Field) -> &str {
        let range = self.offsets.get(field);
        &self.text[range]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FieldOffsets {
    artist: Span,
    album: Span,
    album_artist: Span,
    title: Span,
    genre: Span,
    file: Span,
    dir: Span,
    ext: Span,
    comment: Span,
    composer: Span,
}

impl FieldOffsets {
    fn get(&self, field: Field) -> Range<usize> {
        let span = match field {
            Field::Artist => self.artist,
            Field::Album => self.album,
            Field::AlbumArtist => self.album_artist,
            Field::Title => self.title,
            Field::Genre => self.genre,
            Field::File => self.file,
            Field::Dir => self.dir,
            Field::Ext => self.ext,
            Field::Comment => self.comment,
            Field::Composer => self.composer,
            // Numeric fields have no haystack slice.
            Field::Year | Field::Plays | Field::Duration => Span(0, 0),
        };
        span.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span(usize, usize);

impl From<Span> for Range<usize> {
    fn from(span: Span) -> Self {
        span.0..span.1
    }
}

struct Builder {
    text: String,
}

impl Builder {
    fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    fn push(&mut self, value: Option<&str>) -> Span {
        let start = self.text.len();
        if let Some(value) = value {
            for ch in normalize_text(value).chars() {
                // Sanitize the field separator so offsets stay valid even if
                // metadata happens to contain a control character.
                if ch == FIELD_SEP {
                    self.text.push(' ');
                } else {
                    self.text.push(ch);
                }
            }
        }
        let end = self.text.len();
        self.text.push(FIELD_SEP);
        Span(start, end)
    }
}

#[cfg(test)]
mod tests;
