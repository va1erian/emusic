//! Single-track tag editor dialog (#172).
//!
//! Opened from a track row's context menu ("Edit tags…"), the dialog shows one
//! editable field per scalar tag, validates the numeric ones, treats a blank
//! field as "clear", and offers Revert to the values it opened with. Applying
//! emits an [`EditRequest`] for the shell to hand to the library backend; the
//! per-file [`EditOutcome`]s that come back are rendered in the dialog, so a
//! read-only or missing file is reported where the user was editing.
//!
//! The state lives on [`AppState`](crate::state::AppState) (like the
//! now-playing Properties dialog) rather than inside a track table, because
//! the shell owns the async result channel that feeds the error list.

mod form;
#[cfg(test)]
mod tests;

pub use form::{TagForm, TagFormErrors};

use std::path::PathBuf;
use std::time::Duration;

use eframe::egui;

use crate::library_api::{EditOutcome, EditRequest, TrackInfo};

/// How often the dialog asks for a repaint while an edit is in flight, so the
/// background worker's outcome is picked up even when nothing else is moving.
const PENDING_REPAINT: Duration = Duration::from_millis(100);

/// The one open tag editor, if any.
#[derive(Debug)]
pub struct TagEditorState {
    /// The file being edited; also matches the incoming [`EditOutcome`]s.
    path: PathBuf,
    /// Values the dialog opened with (or last saved), for Revert.
    original: TagForm,
    /// The user's current edits.
    form: TagForm,
    status: Status,
}

/// Where the dialog is in the edit/submit/result cycle.
#[derive(Debug)]
enum Status {
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
struct Failure {
    path: String,
    message: String,
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
        }
    }
}

/// Shows the dialog while an editor is open. Returns the request to submit
/// when the user applies a valid form.
pub fn show(ctx: &egui::Context, state: &mut Option<TagEditorState>) -> Option<EditRequest> {
    let editor = state.as_mut()?;

    let errors = editor.form.validate();
    let pending = matches!(editor.status, Status::Pending);
    let mut apply = false;
    let mut revert = false;
    let mut close = false;

    let modal = egui::Modal::new(egui::Id::new("tag_editor")).show(ctx, |ui| {
        ui.set_width(480.0);
        ui.heading("Edit tags");
        ui.add_space(2.0);
        ui.label(egui::RichText::new(editor.path.display().to_string()).weak());
        ui.add_space(8.0);

        egui::Grid::new("tag_editor_fields")
            .num_columns(2)
            .spacing([16.0, 5.0])
            .show(ui, |ui| {
                text_field(ui, "Title", &mut editor.form.title);
                text_field(ui, "Artist", &mut editor.form.artist);
                text_field(ui, "Album", &mut editor.form.album);
                text_field(ui, "Album artist", &mut editor.form.album_artist);
                text_field(ui, "Genre", &mut editor.form.genre);
                number_field(ui, "Year", &mut editor.form.year, errors.year.as_deref());
                number_field(
                    ui,
                    "Track",
                    &mut editor.form.track_no,
                    errors.track_no.as_deref(),
                );
                number_field(
                    ui,
                    "Disc",
                    &mut editor.form.disc_no,
                    errors.disc_no.as_deref(),
                );
                text_field(ui, "Composer", &mut editor.form.composer);
                text_field(ui, "Comment", &mut editor.form.comment);
            });

        status_line(ui, &editor.status);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!pending && errors.is_empty(), egui::Button::new("Apply"))
                .clicked()
            {
                apply = true;
            }
            if ui.button("Revert").clicked() {
                revert = true;
            }
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    });

    if revert {
        editor.form = editor.original.clone();
        editor.status = Status::Editing;
    }

    let request = if apply {
        editor.form.to_tags().ok().map(|tags| {
            editor.status = Status::Pending;
            EditRequest::new(editor.path.clone(), tags)
        })
    } else {
        None
    };

    if close || modal.should_close() {
        *state = None;
    } else if pending {
        ctx.request_repaint_after(PENDING_REPAINT);
    }
    request
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

/// A labelled single-line text field spanning the value column.
fn text_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(egui::RichText::new(label).strong());
    ui.add(egui::TextEdit::singleline(value).desired_width(300.0));
    ui.end_row();
}

/// A numeric text field, with its validation message below it when invalid.
fn number_field(ui: &mut egui::Ui, label: &str, value: &mut String, error: Option<&str>) {
    ui.label(egui::RichText::new(label).strong());
    ui.vertical(|ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(300.0));
        if let Some(error) = error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    });
    ui.end_row();
}

/// The submit status line: a spinner while in flight, a green confirmation on
/// success, or the per-file failure list.
fn status_line(ui: &mut egui::Ui, status: &Status) {
    match status {
        Status::Editing => {}
        Status::Pending => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Applying…");
            });
        }
        Status::Saved => {
            ui.colored_label(egui::Color32::from_rgb(0x3C, 0xB3, 0x71), "Saved");
        }
        Status::Failed(failures) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                "Some files could not be saved:",
            );
            for failure in failures {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("{}: {}", failure.path, failure.message),
                );
            }
        }
    }
}
