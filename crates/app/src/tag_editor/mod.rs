//! Single-track tag editor dialog (#172): the egui rendering.
//!
//! The editor's state and its edit/submit/result cycle live in
//! `emusic-ui` (moved with the shell, #97); this module only draws the
//! dialog and turns the buttons into the state transitions the shell reads.

pub use emusic_ui::tag_editor::{Failure, Status, TagEditorState, TagForm, TagFormErrors, deliver};

use std::time::Duration;

use eframe::egui;

use crate::library_api::EditRequest;

/// How often the dialog asks for a repaint while an edit is in flight, so the
/// background worker's outcome is picked up even when nothing else is moving.
const PENDING_REPAINT: Duration = Duration::from_millis(100);

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
