//! The tag editor's editable text form and its conversion to the library's
//! [`EditableTags`].
//!
//! Keeping every field as a string lets the user type freely (including a
//! half-entered number) while the numeric fields are validated separately. A
//! blank field maps to `None` — the scanner's "absent, not empty" convention —
//! so clearing a field removes the tag rather than writing an empty string.

use std::path::Path;

use crate::library_api::{Candidate, EditableTags, TrackInfo, TrackQuery};

/// One text field per [`EditableTags`] field, in the order the dialog shows
/// them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagForm {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: String,
    pub track_no: String,
    pub disc_no: String,
    pub composer: String,
    pub comment: String,
}

/// The numeric fields that failed to parse, each with a message to show next
/// to the offending field. Empty when the form is valid.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagFormErrors {
    pub year: Option<String>,
    pub track_no: Option<String>,
    pub disc_no: Option<String>,
}

impl TagFormErrors {
    /// Whether every field parsed.
    pub fn is_empty(&self) -> bool {
        self.year.is_none() && self.track_no.is_none() && self.disc_no.is_none()
    }
}

impl TagForm {
    /// Snapshots a track's current tag values as text.
    pub fn from_track(track: &TrackInfo) -> Self {
        Self {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            album_artist: track.album_artist.clone(),
            genre: track.genre.clone(),
            year: optional_text(track.year),
            track_no: optional_text(track.track_no),
            disc_no: optional_text(track.disc_no),
            composer: track.composer.clone(),
            comment: track.comment.clone(),
        }
    }

    /// Validates the numeric fields without building the tags.
    pub fn validate(&self) -> TagFormErrors {
        TagFormErrors {
            year: number_error::<i32>(&self.year),
            track_no: number_error::<u32>(&self.track_no),
            disc_no: number_error::<u32>(&self.disc_no),
        }
    }

    /// Converts the form to tags, treating a blank field as "clear".
    ///
    /// # Errors
    ///
    /// Returns the same [`TagFormErrors`] as [`TagForm::validate`] when a
    /// numeric field is not a whole number.
    pub fn to_tags(&self) -> Result<EditableTags, TagFormErrors> {
        let errors = self.validate();
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(EditableTags {
            title: text_or_none(&self.title),
            artist: text_or_none(&self.artist),
            album: text_or_none(&self.album),
            album_artist: text_or_none(&self.album_artist),
            genre: text_or_none(&self.genre),
            year: parse_or_none::<i32>(&self.year),
            track_no: parse_or_none::<u32>(&self.track_no),
            disc_no: parse_or_none::<u32>(&self.disc_no),
            composer: text_or_none(&self.composer),
            comment: text_or_none(&self.comment),
        })
    }

    /// Builds the online lookup query from the current form values (#209).
    ///
    /// The title/artist/album come from the fields so the user can seed a
    /// lookup from a partial edit; the file name is carried as a fallback for
    /// when the title is blank.
    pub fn to_query(&self, path: &Path) -> TrackQuery {
        TrackQuery {
            title: text_or_none(&self.title),
            artist: text_or_none(&self.artist),
            album: text_or_none(&self.album),
            duration: None,
            filename: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
        }
    }

    /// Fills the form from a candidate, leaving fields the candidate does not
    /// carry untouched so the user's own edits are preserved.
    pub fn apply_candidate(&mut self, candidate: &Candidate) {
        set_text(&mut self.title, candidate.title.as_deref());
        set_text(&mut self.artist, candidate.artist.as_deref());
        set_text(&mut self.album, candidate.album.as_deref());
        set_text(&mut self.album_artist, candidate.album_artist.as_deref());
        set_text(&mut self.genre, candidate.genre.as_deref());
        set_text(&mut self.composer, candidate.composer.as_deref());
        if let Some(year) = candidate.year {
            self.year = year.to_string();
        }
        if let Some(track_no) = candidate.track_no {
            self.track_no = track_no.to_string();
        }
        if let Some(disc_no) = candidate.disc_no {
            self.disc_no = disc_no.to_string();
        }
    }
}

/// Overwrites `field` with `value` when the candidate carries one.
fn set_text(field: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        *field = value.to_string();
    }
}

/// `value` as text, or an empty string when absent.
fn optional_text<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

/// The trimmed value, or `None` when it is blank.
fn text_or_none(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Parses a non-blank value, mapping a blank (or, defensively, unparsable)
/// field to `None`. Callers validate first, so the defensive arm only guards
/// against a form mutated between validation and conversion.
fn parse_or_none<T: std::str::FromStr>(value: &str) -> Option<T> {
    value.trim().parse().ok()
}

/// `Some(message)` when `value` is non-blank but not a whole number.
fn number_error<T: std::str::FromStr>(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.parse::<T>().is_err()).then(|| "Must be a whole number".to_string())
}
