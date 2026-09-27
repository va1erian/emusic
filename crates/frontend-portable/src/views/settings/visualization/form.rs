//! The Visualization page's timing/audio controls and the user preset folder
//! (#306): preset and soft-cut durations, hard cuts with sensitivity, beat
//! sensitivity, shuffle and the FPS cap, plus the optional folder picker.
//!
//! Every slider shows its current value to the left of the track. Each change
//! maps to a [`VisualizationEdit`] the page turns into a
//! [`Command::Viz`](emusic_ui::state::Command::Viz) settings update.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use emusic_ui::state::projectm::ProjectMSettings;
use xui::xui_core::backend::WidgetId;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, CheckBox, Edit, HasText, Label, Slider};

use super::super::SettingsMsg;
use super::super::form::{FormPage, LABEL_WIDTH, ROW_HEIGHT, RowBuilder};
use crate::app::Msg;

/// Width of a slider's value label, in design units.
const VALUE_WIDTH: f32 = 64.0;
/// Width of the user-folder buttons, in design units.
const BUTTON_WIDTH: f32 = 90.0;

/// Preset duration range, in seconds (mirrors `ProjectMSettings`' clamp).
const DURATION_RANGE: (f64, f64) = (1.0, 3600.0);
/// Soft-cut duration range, in seconds.
const SOFT_CUT_RANGE: (f64, f64) = (0.0, 60.0);
/// Beat/hard-cut sensitivity range.
const SENSITIVITY_RANGE: (f64, f64) = (0.0, 10.0);
/// FPS cap range.
const FPS_RANGE: (f64, f64) = (10.0, 240.0);

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
pub(super) struct TimingForm {
    _duration_label: Label<Msg>,
    duration_value: Label<Msg>,
    duration: Slider<Msg>,
    _soft_label: Label<Msg>,
    soft_value: Label<Msg>,
    soft: Slider<Msg>,
    hard_cuts: CheckBox<Msg>,
    _hard_label: Label<Msg>,
    hard_value: Label<Msg>,
    hard: Slider<Msg>,
    _beat_label: Label<Msg>,
    beat_value: Label<Msg>,
    beat: Slider<Msg>,
    _shuffle: CheckBox<Msg>,
    _fps_label: Label<Msg>,
    fps_value: Label<Msg>,
    fps: Slider<Msg>,
    _user_label: Label<Msg>,
    user_edit: Edit<Msg>,
    _user_browse: Button<Msg>,
    _user_clear: Button<Msg>,
    _user_apply: Button<Msg>,
    /// The user folder last mirrored into the text field.
    applied_user_dir: RefCell<Option<PathBuf>>,
}

impl TimingForm {
    /// Builds the controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(form: &FormPage) -> TimingForm {
        let duration_row = form.row(ROW_HEIGHT).expect("create duration row");
        let duration_label = Label::new(duration_row.ui(), Rect::default(), "Preset duration")
            .expect("create duration label");
        let duration_value =
            Label::new(duration_row.ui(), Rect::default(), "").expect("create duration value");
        let duration = Slider::new(
            duration_row.ui(),
            Rect::default(),
            DURATION_RANGE.0,
            DURATION_RANGE.1,
        )
        .expect("create duration slider")
        .on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::PresetDuration(value),
            )))
        });
        slider_row(
            duration_row,
            duration_label.id(),
            duration_value.id(),
            duration.id(),
        );

        let soft_row = form.row(ROW_HEIGHT).expect("create soft-cut row");
        let soft_label = Label::new(soft_row.ui(), Rect::default(), "Soft cut duration")
            .expect("create soft label");
        let soft_value = Label::new(soft_row.ui(), Rect::default(), "").expect("create soft value");
        let soft = Slider::new(
            soft_row.ui(),
            Rect::default(),
            SOFT_CUT_RANGE.0,
            SOFT_CUT_RANGE.1,
        )
        .expect("create soft slider")
        .on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::SoftCut(
                value,
            ))))
        });
        slider_row(soft_row, soft_label.id(), soft_value.id(), soft.id());

        let hard_cuts = CheckBox::new(form.ui(), Rect::default(), "Switch preset on a loud beat")
            .expect("create hard-cuts checkbox")
            .on_toggle(|on| {
                Some(Msg::Settings(SettingsMsg::Viz(
                    VisualizationEdit::HardCuts(on),
                )))
            });
        form.add_full(hard_cuts.id(), ROW_HEIGHT);

        let hard_row = form.row(ROW_HEIGHT).expect("create hard-cut row");
        let hard_label = Label::new(hard_row.ui(), Rect::default(), "Hard cut sensitivity")
            .expect("create hard-cut label");
        let hard_value =
            Label::new(hard_row.ui(), Rect::default(), "").expect("create hard-cut value");
        let hard = Slider::new(
            hard_row.ui(),
            Rect::default(),
            SENSITIVITY_RANGE.0,
            SENSITIVITY_RANGE.1,
        )
        .expect("create hard-cut slider")
        .on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::HardCutSensitivity(value),
            )))
        });
        slider_row(hard_row, hard_label.id(), hard_value.id(), hard.id());

        let beat_row = form.row(ROW_HEIGHT).expect("create beat row");
        let beat_label = Label::new(beat_row.ui(), Rect::default(), "Beat sensitivity")
            .expect("create beat label");
        let beat_value = Label::new(beat_row.ui(), Rect::default(), "").expect("create beat value");
        let beat = Slider::new(
            beat_row.ui(),
            Rect::default(),
            SENSITIVITY_RANGE.0,
            SENSITIVITY_RANGE.1,
        )
        .expect("create beat slider")
        .on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Viz(
                VisualizationEdit::BeatSensitivity(value),
            )))
        });
        slider_row(beat_row, beat_label.id(), beat_value.id(), beat.id());

        let shuffle = CheckBox::new(form.ui(), Rect::default(), "Shuffle preset order")
            .expect("create shuffle checkbox")
            .on_toggle(|on| {
                Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::Shuffle(
                    on,
                ))))
            });
        form.add_full(shuffle.id(), ROW_HEIGHT);

        let fps_row = form.row(ROW_HEIGHT).expect("create fps row");
        let fps_label =
            Label::new(fps_row.ui(), Rect::default(), "FPS cap").expect("create fps label");
        let fps_value = Label::new(fps_row.ui(), Rect::default(), "").expect("create fps value");
        let fps = Slider::new(fps_row.ui(), Rect::default(), FPS_RANGE.0, FPS_RANGE.1)
            .expect("create fps slider")
            .on_change(|value| {
                Some(Msg::Settings(SettingsMsg::Viz(VisualizationEdit::FpsCap(
                    value,
                ))))
            });
        slider_row(fps_row, fps_label.id(), fps_value.id(), fps.id());

        let user_row = form.row(ROW_HEIGHT).expect("create user preset row");
        let user_label = Label::new(user_row.ui(), Rect::default(), "User preset folder")
            .expect("create user label");
        let user_edit =
            Edit::new(user_row.ui(), Rect::default(), "").expect("create user preset field");
        let user_browse = Button::new(user_row.ui(), Rect::default(), "Browse...")
            .expect("create user browse")
            .on_click(|| Some(Msg::Settings(SettingsMsg::VizBrowse)));
        let user_clear = Button::new(user_row.ui(), Rect::default(), "Clear")
            .expect("create user clear")
            .on_click(|| Some(Msg::Settings(SettingsMsg::VizClearUserDir)));
        let user_apply = Button::new(user_row.ui(), Rect::default(), "Apply")
            .expect("create user apply")
            .on_click(|| Some(Msg::Settings(SettingsMsg::VizCommitUserDir)));
        user_row
            .fixed(user_label.id(), LABEL_WIDTH)
            .fill(user_edit.id())
            .fixed(user_browse.id(), dip(BUTTON_WIDTH))
            .fixed(user_clear.id(), dip(BUTTON_WIDTH))
            .fixed(user_apply.id(), dip(BUTTON_WIDTH))
            .finish();

        TimingForm {
            _duration_label: duration_label,
            duration_value,
            duration,
            _soft_label: soft_label,
            soft_value,
            soft,
            hard_cuts,
            _hard_label: hard_label,
            hard_value,
            hard,
            _beat_label: beat_label,
            beat_value,
            beat,
            _shuffle: shuffle,
            _fps_label: fps_label,
            fps_value,
            fps,
            _user_label: user_label,
            user_edit,
            _user_browse: user_browse,
            _user_clear: user_clear,
            _user_apply: user_apply,
            applied_user_dir: RefCell::new(None),
        }
    }

    /// Mirrors the settings onto the controls.
    pub(super) fn sync(&self, settings: &ProjectMSettings) {
        self.duration
            .set_value(f64::from(settings.preset_duration_secs));
        self.duration_value
            .set_text(&seconds(settings.preset_duration_secs));
        self.soft.set_value(f64::from(settings.soft_cut_secs));
        self.soft_value.set_text(&seconds(settings.soft_cut_secs));
        self.hard_cuts.set_checked(settings.hard_cuts);
        self.hard.set_enabled(settings.hard_cuts);
        self.hard
            .set_value(f64::from(settings.hard_cut_sensitivity));
        self.hard_value
            .set_text(&number(settings.hard_cut_sensitivity));
        self.beat.set_value(f64::from(settings.beat_sensitivity));
        self.beat_value.set_text(&number(settings.beat_sensitivity));
        self.fps.set_value(f64::from(settings.fps_cap));
        self.fps_value.set_text(&number(settings.fps_cap as f32));

        if self.applied_user_dir.borrow().as_ref() != settings.user_preset_dir.as_ref() {
            *self.applied_user_dir.borrow_mut() = settings.user_preset_dir.clone();
            self.user_edit
                .set_text(&path_text(settings.user_preset_dir.as_deref()));
        }
    }

    /// The user preset path typed into the field, or `None` when empty.
    pub(super) fn user_dir_text(&self) -> Option<PathBuf> {
        parse_path(&self.user_edit.text())
    }

    /// Replaces the user-folder field's text (after the picker returns).
    pub(super) fn set_user_dir_text(&self, path: Option<&Path>) {
        self.user_edit.set_text(&path_text(path));
    }
}

/// Finishes a "label | value | slider" row.
fn slider_row(row: RowBuilder<'_>, label: WidgetId, value: WidgetId, slider: WidgetId) {
    row.fixed(label, LABEL_WIDTH)
        .fixed(value, dip(VALUE_WIDTH))
        .fill(slider)
        .finish();
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

/// The display text for a path, or empty when unset.
fn path_text(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_default()
}

/// Trims a typed path and turns an empty one into `None`.
fn parse_path(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim().trim_matches('"');
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
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
