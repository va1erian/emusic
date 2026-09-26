#![forbid(unsafe_code)]

//! The Visualization page's timing/audio controls and the user preset folder
//! (#306): preset and soft-cut durations, hard cuts with sensitivity, beat
//! sensitivity, shuffle and the FPS cap, plus the optional folder picker.
//!
//! Every slider shows its current value to the left of the track, and every
//! control carries a tooltip explaining what it does. Each slider or toggle
//! change maps to a [`VisualizationEdit`] the page turns into a
//! [`Command::Viz`](emusic_ui::state::Command::Viz) settings update.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use emusic_ui::state::projectm::ProjectMSettings;
use xui::prelude::*;
use xui::{Button, CheckBox, Edit};

use crate::app::Msg;

use super::super::{FormRow, LABEL_WIDTH, ROW_HEIGHT, SettingsMsg};

/// Width of the user-folder field, in design units.
const FIELD_WIDTH: f32 = 340.0;
/// Width of a slider's value label, in design units.
const VALUE_WIDTH: f32 = 64.0;
/// Preset duration range, in seconds (mirrors `ProjectMSettings`' clamp).
const DURATION_RANGE: std::ops::RangeInclusive<f64> = 1.0..=3600.0;
/// Soft-cut duration range, in seconds.
const SOFT_CUT_RANGE: std::ops::RangeInclusive<f64> = 0.0..=60.0;
/// Beat/hard-cut sensitivity range.
const SENSITIVITY_RANGE: std::ops::RangeInclusive<f64> = 0.0..=10.0;
/// FPS cap range.
const FPS_RANGE: std::ops::RangeInclusive<f64> = 10.0..=240.0;

/// Tooltip for the preset-duration slider.
const TIP_DURATION: &str = "How long each preset plays before switching to the next one.";
/// Tooltip for the soft-cut-duration slider.
const TIP_SOFT_CUT: &str = "Seconds of cross-fade when blending into the next preset.";
/// Tooltip for the hard-cuts checkbox.
const TIP_HARD_CUTS: &str = "Cut to the next preset as soon as a loud beat is detected, instead of waiting for the preset duration.";
/// Tooltip for the hard-cut-sensitivity slider.
const TIP_HARD_CUT_SENSITIVITY: &str =
    "How strong a beat must be to trigger a hard cut; higher values cut less often.";
/// Tooltip for the beat-sensitivity slider.
const TIP_BEAT_SENSITIVITY: &str =
    "How strongly presets react to beats; higher values react to softer beats.";
/// Tooltip for the shuffle checkbox.
const TIP_SHUFFLE: &str = "Play presets in a random order instead of the order of the folders.";
/// Tooltip for the FPS-cap slider.
const TIP_FPS: &str = "Upper limit on visualization frames per second; lower it to save GPU.";
/// Tooltip for the user preset folder field.
const TIP_USER_DIR: &str = "An extra folder of your own .milk presets, scanned recursively.";
/// Tooltip for the "Browse..." button.
const TIP_BROWSE: &str = "Pick a folder of your own .milk presets.";
/// Tooltip for the "Clear" button.
const TIP_CLEAR: &str = "Stop using the user preset folder.";

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
    duration_value: Label,
    duration: Slider<Msg>,
    soft_cut_label: Label,
    soft_cut_value: Label,
    soft_cut: Slider<Msg>,
    hard_cuts: CheckBox<Msg>,
    hard_cut_label: Label,
    hard_cut_value: Label,
    hard_cut: Slider<Msg>,
    beat_label: Label,
    beat_value: Label,
    beat: Slider<Msg>,
    shuffle: CheckBox<Msg>,
    fps_label: Label,
    fps_value: Label,
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
    pub(super) fn new(ui: &mut Ui<Msg>) -> xui::Result<Self> {
        let duration = Slider::new(ui, DURATION_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::PresetDuration(value),
            )))
        });
        duration.set_tooltip(TIP_DURATION);
        let soft_cut = Slider::new(ui, SOFT_CUT_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::SoftCut(
                value,
            ))))
        });
        soft_cut.set_tooltip(TIP_SOFT_CUT);
        let hard_cuts = CheckBox::new(ui, "Switch preset on a loud beat")?.on_toggle(|on| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::HardCuts(on),
            )))
        });
        hard_cuts.set_tooltip(TIP_HARD_CUTS);
        let hard_cut = Slider::new(ui, SENSITIVITY_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::HardCutSensitivity(value),
            )))
        });
        hard_cut.set_tooltip(TIP_HARD_CUT_SENSITIVITY);
        let beat = Slider::new(ui, SENSITIVITY_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::BeatSensitivity(value),
            )))
        });
        beat.set_tooltip(TIP_BEAT_SENSITIVITY);
        let shuffle = CheckBox::new(ui, "Shuffle preset order")?.on_toggle(|on| {
            Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::Shuffle(
                on,
            ))))
        });
        shuffle.set_tooltip(TIP_SHUFFLE);
        let fps = Slider::new(ui, FPS_RANGE)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::FpsCap(
                value,
            ))))
        });
        fps.set_tooltip(TIP_FPS);
        let user_edit = Edit::single_line(ui)?
            .cue("optional folder of your own .milk presets")
            .on_submit(|| Some(Msg::Settings(SettingsMsg::VizCommitUserDir)))
            .on_focus(|focused| (!focused).then_some(Msg::Settings(SettingsMsg::VizCommitUserDir)));
        user_edit.set_tooltip(TIP_USER_DIR);
        let user_browse =
            Button::new(ui, "Browse...")?.on_click(|| Some(Msg::Settings(SettingsMsg::VizBrowse)));
        user_browse.set_tooltip(TIP_BROWSE);
        let user_clear = Button::new(ui, "Clear")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::VizClearUserDir)));
        user_clear.set_tooltip(TIP_CLEAR);

        let form = Self {
            duration_label: label(ui, "Preset duration", TIP_DURATION)?,
            duration_value: Label::new(ui, Rect::default(), "")?,
            duration,
            soft_cut_label: label(ui, "Soft cut duration", TIP_SOFT_CUT)?,
            soft_cut_value: Label::new(ui, Rect::default(), "")?,
            soft_cut,
            hard_cuts,
            hard_cut_label: label(ui, "Hard cut sensitivity", TIP_HARD_CUT_SENSITIVITY)?,
            hard_cut_value: Label::new(ui, Rect::default(), "")?,
            hard_cut,
            beat_label: label(ui, "Beat sensitivity", TIP_BEAT_SENSITIVITY)?,
            beat_value: Label::new(ui, Rect::default(), "")?,
            beat,
            shuffle,
            fps_label: label(ui, "FPS cap", TIP_FPS)?,
            fps_value: Label::new(ui, Rect::default(), "")?,
            fps,
            user_label: label(ui, "User preset folder", TIP_USER_DIR)?,
            user_edit,
            user_browse,
            user_clear,
            hard_cuts_on: Cell::new(false),
            applied_user_dir: RefCell::new(None),
        };
        // The sensitivity slider is only part of the form while hard cuts is
        // on. Hide it at construction so it never floats unlaid-out on the
        // page when the persisted setting starts off (#306 follow-up).
        form.set_hard_cuts(false);
        Ok(form)
    }

    /// The form's rows, in display order. The sensitivity row is only part of
    /// the form while hard cuts is on, like the tracker page's dependent rows.
    pub(super) fn rows(&self) -> Vec<FormRow> {
        let mut rows = vec![
            slider_row(
                &self.duration_label,
                &self.duration_value,
                self.duration.fill(1),
            ),
            slider_row(
                &self.soft_cut_label,
                &self.soft_cut_value,
                self.soft_cut.fill(1),
            ),
            (self.hard_cuts.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
        ];
        if self.hard_cuts_on.get() {
            rows.push(slider_row(
                &self.hard_cut_label,
                &self.hard_cut_value,
                self.hard_cut.fill(1),
            ));
        }
        rows.push(slider_row(
            &self.beat_label,
            &self.beat_value,
            self.beat.fill(1),
        ));
        rows.push((self.shuffle.height(dip(ROW_HEIGHT)), ROW_HEIGHT));
        rows.push(slider_row(
            &self.fps_label,
            &self.fps_value,
            self.fps.fill(1),
        ));
        rows
    }

    /// The user preset folder row, placed with the pack controls.
    pub(super) fn user_rows(&self) -> Vec<FormRow> {
        vec![(
            row![
                self.user_label.width(dip(LABEL_WIDTH)),
                Layout::row()
                    .spacing(dip(8.0))
                    .item(self.user_edit.width(dip(FIELD_WIDTH)))
                    .item(&self.user_browse)
                    .item(&self.user_clear)
                    .height(dip(ROW_HEIGHT))
            ]
            .height(dip(ROW_HEIGHT)),
            ROW_HEIGHT,
        )]
    }

    /// Mirrors the settings onto the controls. Returns whether the hard-cut
    /// row's visibility changed, so the page can reinstall the form.
    pub(super) fn sync(&self, settings: &ProjectMSettings) -> bool {
        self.duration
            .set_value(f64::from(settings.preset_duration_secs));
        self.duration_value
            .set_text(&seconds(settings.preset_duration_secs));
        self.soft_cut.set_value(f64::from(settings.soft_cut_secs));
        self.soft_cut_value
            .set_text(&seconds(settings.soft_cut_secs));
        self.hard_cuts.set_checked(settings.hard_cuts);
        self.hard_cut
            .set_value(f64::from(settings.hard_cut_sensitivity));
        self.hard_cut_value
            .set_text(&number(settings.hard_cut_sensitivity));
        self.beat.set_value(f64::from(settings.beat_sensitivity));
        self.beat_value.set_text(&number(settings.beat_sensitivity));
        self.shuffle.set_checked(settings.shuffle);
        self.fps.set_value(f64::from(settings.fps_cap));
        self.fps_value.set_text(&number(settings.fps_cap as f32));

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
        self.hard_cut_value.set_visible(on);
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

/// A form row of "label | value | slider", with the current value shown to the
/// left of the slider track.
fn slider_row(label: &Label, value: &Label, slider: LayoutItem) -> FormRow {
    (
        row![
            label.width(dip(LABEL_WIDTH)),
            value.width(dip(VALUE_WIDTH)),
            slider
        ]
        .height(dip(ROW_HEIGHT)),
        ROW_HEIGHT,
    )
}

/// Builds a plain label that also explains itself on hover.
fn label(ui: &mut Ui<Msg>, text: &str, tooltip: &str) -> xui::Result<Label> {
    let label = Label::new(ui, Rect::default(), text)?;
    label.set_tooltip(tooltip);
    Ok(label)
}

/// A value's text with a seconds unit.
fn seconds(value: f32) -> String {
    format!("{} s", number(value))
}

/// A value's text, without a pointless trailing `.0`.
fn number(value: f32) -> String {
    if value.fract().abs() < f32::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
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

    #[test]
    fn values_drop_a_pointless_trailing_zero() {
        assert_eq!(number(30.0), "30");
        assert_eq!(number(2.5), "2.5");
        assert_eq!(seconds(3.0), "3 s");
        assert_eq!(seconds(2.5), "2.5 s");
    }
}
