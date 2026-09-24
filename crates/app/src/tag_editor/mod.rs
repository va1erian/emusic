//! Single-track tag editor dialog (#172): the egui rendering.
//!
//! The editor's state and its edit/submit/result cycle live in
//! `emusic-ui` (moved with the shell, #97); this module only draws the
//! dialog and turns the buttons into the state transitions the shell reads.

pub use emusic_ui::tag_editor::{
    AutoTagState, Failure, Status, TagEditorState, TagForm, TagFormErrors, deliver,
    deliver_auto_tag,
};

use std::time::Duration;

use eframe::egui;

use crate::library_api::{AutoTagRequest, Candidate, EditRequest};

/// How often the dialog asks for a repaint while an edit or lookup is in
/// flight, so the background worker's outcome is picked up even when nothing
/// else is moving.
const PENDING_REPAINT: Duration = Duration::from_millis(100);

/// What the dialog wants the app to do after a frame (#209).
pub enum TagEditorAction {
    /// Submit the form as a tag edit.
    Apply(EditRequest),
    /// Run an online metadata lookup seeded from the current form.
    AutoTag(AutoTagRequest),
}

/// Shows the dialog while an editor is open. Returns the request to submit
/// when the user applies a valid form, or the lookup to run when they click
/// Auto-tag.
pub fn show(ctx: &egui::Context, state: &mut Option<TagEditorState>) -> Option<TagEditorAction> {
    let editor = state.as_mut()?;

    let errors = editor.form.validate();
    let pending = matches!(editor.status, Status::Pending);
    let searching = matches!(editor.auto_tag, AutoTagState::Searching);
    let mut apply = false;
    let mut revert = false;
    let mut close = false;
    let mut auto_tag = false;
    let mut picked = None;

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

        ui.add_space(6.0);
        if ui
            .add_enabled(!pending && !searching, egui::Button::new("Auto-tag"))
            .on_hover_text("Look this track up online and fill the fields from a match")
            .clicked()
        {
            auto_tag = true;
        }
        picked = auto_tag_section(ui, &editor.auto_tag);

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

    if let Some(index) = picked {
        let candidate = match &editor.auto_tag {
            AutoTagState::Matches(candidates) => candidates.get(index).cloned(),
            _ => None,
        };
        if let Some(candidate) = candidate {
            editor.form.apply_candidate(&candidate);
        }
    }

    let action = if apply {
        editor.form.to_tags().ok().map(|tags| {
            editor.status = Status::Pending;
            TagEditorAction::Apply(EditRequest::new(editor.path.clone(), tags))
        })
    } else if auto_tag {
        editor.auto_tag = AutoTagState::Searching;
        Some(TagEditorAction::AutoTag(AutoTagRequest {
            path: editor.path.clone(),
            query: editor.form.to_query(&editor.path),
        }))
    } else {
        None
    };

    let searching = matches!(editor.auto_tag, AutoTagState::Searching);
    if close || modal.should_close() {
        *state = None;
    } else if pending || searching {
        ctx.request_repaint_after(PENDING_REPAINT);
    }
    action
}

/// Draws the lookup state under the Auto-tag button and returns the index of
/// the candidate the user asked to use, if any.
fn auto_tag_section(ui: &mut egui::Ui, state: &AutoTagState) -> Option<usize> {
    match state {
        AutoTagState::Idle => None,
        AutoTagState::Searching => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Searching MusicBrainz…");
            });
            None
        }
        AutoTagState::Matches(candidates) => {
            let mut picked = None;
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Matches").strong());
            for (index, candidate) in candidates.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button("Use this").clicked() {
                        picked = Some(index);
                    }
                    ui.label(candidate_summary(candidate));
                });
            }
            picked
        }
        AutoTagState::NoMatch => {
            ui.label(egui::RichText::new("No match found.").weak());
            None
        }
        AutoTagState::Failed(message) => {
            ui.colored_label(ui.visuals().error_fg_color, message);
            None
        }
    }
}

/// A one-line description of a candidate, e.g.
/// `"Daft Punk — Around the World · Homework (1997) 96%"`.
fn candidate_summary(candidate: &Candidate) -> String {
    let mut summary = match (&candidate.artist, &candidate.title) {
        (Some(artist), Some(title)) => format!("{artist} — {title}"),
        (Some(artist), None) => artist.clone(),
        (None, Some(title)) => title.clone(),
        (None, None) => "(unknown)".to_string(),
    };
    if let Some(album) = &candidate.album {
        summary.push_str(&format!(" · {album}"));
    }
    if let Some(year) = candidate.year {
        summary.push_str(&format!(" ({year})"));
    }
    if candidate.score > 0.0 {
        summary.push_str(&format!("  {:.0}%", candidate.score * 100.0));
    }
    summary
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
