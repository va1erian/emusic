//! Single-track tag editor state (#172, #97).
//!
//! The dialog's editable form and its edit/submit/result cycle live here,
//! independent of the frontend that draws the fields. The egui frontend's
//! `show` reads and mutates these fields; the shell drains the backend's
//! per-file outcomes with [`deliver`].

mod form;
#[cfg(test)]
mod tests;

pub use form::{TagForm, TagFormErrors};

use std::path::PathBuf;

use crate::library_api::{AutoTagError, AutoTagOutcome, Candidate, EditOutcome, TrackInfo};

/// The one open tag editor, if any.
#[derive(Debug)]
pub struct TagEditorState {
    /// The file being edited; also matches the incoming [`EditOutcome`]s.
    pub path: PathBuf,
    /// Values the dialog opened with (or last saved), for Revert.
    pub original: TagForm,
    /// The user's current edits.
    pub form: TagForm,
    /// Where the dialog is in the edit/submit/result cycle.
    pub status: Status,
    /// Where the dialog's online auto-tag lookup is (#208).
    pub auto_tag: AutoTagState,
}

/// Where the dialog's online auto-tag lookup is (#208).
///
/// The lookup runs in the library backend; the shell folds its outcome back in
/// with [`deliver_auto_tag`].
#[derive(Debug, Default)]
pub enum AutoTagState {
    /// No lookup has run in this dialog session.
    #[default]
    Idle,
    /// A lookup is in flight; the backend will report an outcome.
    Searching,
    /// Candidates came back, best first.
    Matches(Vec<Candidate>),
    /// The lookup completed without any match.
    NoMatch,
    /// The lookup failed; the message is shown inline.
    Failed(String),
}

/// Where the dialog is in the edit/submit/result cycle.
#[derive(Debug)]
pub enum Status {
    /// Editing, nothing submitted this session.
    Editing,
    /// A request is with the backend; awaiting its outcome.
    Pending,
    /// The last submit succeeded.
    Saved,
    /// The last submit failed for at least one file.
    Failed(Vec<Failure>),
}

/// One file's write error, kept as plain text so it can outlive the
/// [`EditOutcome`] it came from.
#[derive(Debug)]
pub struct Failure {
    pub path: String,
    pub message: String,
}

impl TagEditorState {
    /// Opens an editor for `track`, seeded from its current tag values.
    pub fn new(track: &TrackInfo) -> Self {
        let form = TagForm::from_track(track);
        Self {
            path: PathBuf::from(&track.path),
            original: form.clone(),
            form,
            status: Status::Editing,
            auto_tag: AutoTagState::Idle,
        }
    }
}

/// Folds auto-tag outcomes into the open editor, if one is showing the
/// affected file. Outcomes for other files are ignored here.
pub fn deliver_auto_tag(state: &mut Option<TagEditorState>, outcomes: Vec<AutoTagOutcome>) {
    let Some(editor) = state.as_mut() else {
        return;
    };
    for outcome in outcomes {
        if outcome.path != editor.path {
            continue;
        }
        editor.auto_tag = match outcome.result {
            Ok(candidates) if candidates.is_empty() => AutoTagState::NoMatch,
            Ok(candidates) => AutoTagState::Matches(candidates),
            Err(AutoTagError::Cancelled) => AutoTagState::Idle,
            Err(error) => AutoTagState::Failed(error.to_string()),
        };
    }
}

/// Folds tag-edit outcomes into the open editor, if one is showing the
/// affected file. Outcomes for other files (e.g. a later batch edit) are
/// ignored here.
pub fn deliver(state: &mut Option<TagEditorState>, outcomes: Vec<EditOutcome>) {
    let Some(editor) = state.as_mut() else {
        return;
    };
    let mut failures = Vec::new();
    let mut succeeded = false;
    for outcome in outcomes {
        if outcome.path != editor.path {
            continue;
        }
        match outcome.result {
            Ok(()) => succeeded = true,
            Err(error) => failures.push(Failure {
                path: outcome.path.display().to_string(),
                message: error.to_string(),
            }),
        }
    }

    if !failures.is_empty() {
        editor.status = Status::Failed(failures);
    } else if succeeded {
        editor.original = editor.form.clone();
        editor.status = Status::Saved;
    }
}
