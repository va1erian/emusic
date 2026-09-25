#![forbid(unsafe_code)]

//! The Visualization page's timing/audio controls and the user preset folder
//! (#306): preset and soft-cut durations, hard cuts with sensitivity, beat
//! sensitivity, shuffle and the FPS cap, plus the optional folder picker.
//!
//! Every slider or toggle change maps to a [`VisualizationEdit`] the page
//! turns into a [`Command::Viz`](emusic_ui::state::Command::Viz) settings
//! update.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use emusic_ui::state::projectm::ProjectMSettings;
use win32ui::prelude::*;
use win32ui::{Button, CheckBox, Edit};

use crate::app::Msg;

use super::super::{FormRow, ROW_HEIGHT, SettingsMsg, labelled};

/// Width of the user-folder field, in design units.
const FIELD_WIDTH: f32 = 340.0;
/// Preset duration range, in seconds (mirrors `ProjectMSettings`' clamp).
const DURATION_RANGE: std::ops::RangeInclusive<f64> = 1.0..=3600.0;
/// Soft-cut duration range, in seconds.
const SOFT_CUT_RANGE: std::ops::RangeInclusive<f64> = 0.0..=60.0;
/// Beat/hard-cut sensitivity range.
const SENSITIVITY_RANGE: std::ops::RangeInclusive<f64> = 0.0..=10.0;
/// FPS cap range.
const FPS_RANGE: std::ops::RangeInclusive<f64> = 10.0..=240.0;

/// One settings field changed on the Visualization page.
pub enum VisualizationEdit {
    PresetDuration(f64),
    SoftCut(f64),
    HardCuts(bool),
    HardCutSensitivity(f64),
    BeatSensitivity(f64),
    Shuffle(bool),
    FpsCap(f64),
}

/// The timing/audio form rows and the user preset folder.
pub(super) struct Form {
    duration_label: Label,
    duration: Slider<Msg>,
    soft_cut_label: Label,
    soft_cut: Slider<Msg>,
    hard_cuts: CheckBox<Msg>,
    hard_cut_label: Label,
    hard_cut: Slider<Msg>,
    beat_label: Label,
    beat: Slider<Msg>,
    shuffle: CheckBox<Msg>,
    fps_label: Label,
    fps: Slider<Msg>,
    user_label: Label,
    user_edit: Edit<Msg>,
    user_browse: Button<Msg>,
    user_clear: Button<Msg>,
    /// Whether hard cuts is on, which controls the sensitivity row.
    hard_cuts_on: Cell<bool>,
    /// The user folder last mirrored into the text field.
    applied_user_dir: RefCell<Option<PathBuf>>,
}

impl Form {
    /// Builds the controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let duration = Slider::new(ui, DURATION_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::PresetDuration(value),
            )))
        });
        let soft_cut = Slider::new(ui, SOFT_CUT_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::SoftCut(
                value,
            ))))
        });
        let hard_cuts = CheckBox::new(ui, "Switch preset on a loud beat")?.on_toggle(|on| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::HardCuts(on),
            )))
        });
        let hard_cut = Slider::new(ui, SENSITIVITY_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::HardCutSensitivity(value),
            )))
        });
        let beat = Slider::new(ui, SENSITIVITY_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::BeatSensitivity(value),
            )))
        });
        let shuffle = CheckBox::new(ui, "Shuffle preset order")?.on_toggle(|on| {
            Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::Shuffle(
                on,
            ))))
        });
        let fps = Slider::new(ui, FPS_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::FpsCap(
                value,
            ))))
        });
        let user_edit = Edit::single_line(ui)?
            .cue("optional folder of your own .milk presets")
            .on_submit(|| Some(Msg::Settings(SettingsMsg::VizCommitUserDir)))
            .on_focus(|focused| (!focused).then_some(Msg::Settings(SettingsMsg::VizCommitUserDir)));
        let user_browse =
            Button::new(ui, "Browse...")?.on_click(|| Some(Msg::Settings(SettingsMsg::VizBrowse)));
        let user_clear = Button::new(ui, "Clear")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::VizClearUserDir)));

        Ok(Self {
            duration_label: Label::new(ui, Rect::default(), "Preset duration")?,
            duration,
            soft_cut_label: Label::new(ui, Rect::default(), "Soft cut duration")?,
            soft_cut,
            hard_cuts,
            hard_cut_label: Label::new(ui, Rect::default(), "Hard cut sensitivity")?,
            hard_cut,
            beat_label: Label::new(ui, Rect::default(), "Beat sensitivity")?,
            beat,
            shuffle,
            fps_label: Label::new(ui, Rect::default(), "FPS cap")?,
            fps,
            user_label: Label::new(ui, Rect::default(), "User preset folder")?,
            user_edit,
            user_browse,
            user_clear,
            hard_cuts_on: Cell::new(false),
            applied_user_dir: RefCell::new(None),
        })
    }

    /// The form's rows, in display order. The sensitivity row is only part of
    /// the form while hard cuts is on, like the tracker page's dependent rows.
    pub(super) fn rows(&self) -> Vec<FormRow> {
        let mut rows = vec![
            (
                labelled(&self.duration_label, self.duration.fill(1)),
                ROW_HEIGHT,
            ),
            (
                labelled(&self.soft_cut_label, self.soft_cut.fill(1)),
                ROW_HEIGHT,
            ),
            (self.hard_cuts.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
        ];
        if self.hard_cuts_on.get() {
            rows.push((
                labelled(&self.hard_cut_label, self.hard_cut.fill(1)),
                ROW_HEIGHT,
            ));
        }
        rows.extend([
            (labelled(&self.beat_label, self.beat.fill(1)), ROW_HEIGHT),
            (self.shuffle.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (labelled(&self.fps_label, self.fps.fill(1)), ROW_HEIGHT),
        ]);
        rows
    }

    /// The user preset folder row, placed with the pack controls.
    pub(super) fn user_rows(&self) -> Vec<FormRow> {
        vec![(
            labelled(
                &self.user_label,
                Layout::row()
                    .spacing(dip(8.0))
                    .item(self.user_edit.width(dip(FIELD_WIDTH)))
                    .item(&self.user_browse)
                    .item(&self.user_clear)
                    .height(dip(ROW_HEIGHT)),
            ),
            ROW_HEIGHT,
        )]
    }

    /// Mirrors the settings onto the controls. Returns whether the hard-cut
    /// row's visibility changed, so the page can reinstall the form.
    pub(super) fn sync(&self, settings: &ProjectMSettings) -> bool {
        self.duration
            .set_value(f64::from(settings.preset_duration_secs));
        self.soft_cut.set_value(f64::from(settings.soft_cut_secs));
        self.hard_cuts.set_checked(settings.hard_cuts);
        self.hard_cut
            .set_value(f64::from(settings.hard_cut_sensitivity));
        self.beat.set_value(f64::from(settings.beat_sensitivity));
        self.shuffle.set_checked(settings.shuffle);
        self.fps.set_value(f64::from(settings.fps_cap));

        if self.applied_user_dir.borrow().as_ref() != settings.user_preset_dir.as_ref() {
            *self.applied_user_dir.borrow_mut() = settings.user_preset_dir.clone();
            self.user_edit
                .set_text(&path_text(settings.user_preset_dir.as_deref()));
        }

        if self.hard_cuts_on.get() != settings.hard_cuts {
            self.set_hard_cuts(settings.hard_cuts);
            return true;
        }
        false
    }

    /// Updates the hard-cut flag and its dependent row's visibility.
    pub(super) fn set_hard_cuts(&self, on: bool) {
        self.hard_cuts_on.set(on);
        self.hard_cut_label.set_visible(on);
        self.hard_cut.set_visible(on);
        self.hard_cut.set_enabled(on);
    }

    /// The user preset path typed into the field, or `None` when empty.
    pub(super) fn user_dir_text(&self) -> Option<PathBuf> {
        parse_path(&self.user_edit.text())
    }

    /// Replaces the user-folder field's text.
    pub(super) fn set_user_dir_text(&self, path: Option<&PathBuf>) {
        self.user_edit
            .set_text(&path_text(path.map(PathBuf::as_path)));
    }
}

/// Merges one field change into `settings`.
pub(super) fn apply_edit(edit: &VisualizationEdit, settings: &mut ProjectMSettings) {
    match edit {
        VisualizationEdit::PresetDuration(value) => settings.preset_duration_secs = *value as f32,
        VisualizationEdit::SoftCut(value) => settings.soft_cut_secs = *value as f32,
        VisualizationEdit::HardCuts(on) => settings.hard_cuts = *on,
        VisualizationEdit::HardCutSensitivity(value) => {
            settings.hard_cut_sensitivity = *value as f32;
        }
        VisualizationEdit::BeatSensitivity(value) => settings.beat_sensitivity = *value as f32,
        VisualizationEdit::Shuffle(on) => settings.shuffle = *on,
        VisualizationEdit::FpsCap(value) => settings.fps_cap = value.round() as u32,
    }
}

/// The display text for a path, or empty when unset.
fn path_text(path: Option<&std::path::Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_default()
}

/// Trims a typed path and turns an empty one into `None`.
fn parse_path(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim().trim_matches('"');
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_path_trims_whitespace_and_quotes() {
        assert_eq!(
            parse_path(r#"  "C:\presets\mine"  "#),
            Some(PathBuf::from(r"C:\presets\mine"))
        );
        assert_eq!(parse_path("   "), None);
    }

    #[test]
    fn each_edit_lands_on_its_field() {
        let mut settings = ProjectMSettings::default();
        apply_edit(&VisualizationEdit::PresetDuration(45.0), &mut settings);
        apply_edit(&VisualizationEdit::SoftCut(2.5), &mut settings);
        apply_edit(&VisualizationEdit::HardCuts(true), &mut settings);
        apply_edit(&VisualizationEdit::HardCutSensitivity(3.0), &mut settings);
        apply_edit(&VisualizationEdit::BeatSensitivity(4.0), &mut settings);
        apply_edit(&VisualizationEdit::Shuffle(false), &mut settings);
        apply_edit(&VisualizationEdit::FpsCap(144.4), &mut settings);
        assert_eq!(settings.preset_duration_secs, 45.0);
        assert_eq!(settings.soft_cut_secs, 2.5);
        assert!(settings.hard_cuts);
        assert_eq!(settings.hard_cut_sensitivity, 3.0);
        assert_eq!(settings.beat_sensitivity, 4.0);
        assert!(!settings.shuffle);
        assert_eq!(settings.fps_cap, 144);
    }
}
