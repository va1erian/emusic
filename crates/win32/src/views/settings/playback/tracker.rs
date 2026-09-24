//! Playback page → tracker module settings: interpolation, ramping, stereo
//! separation, amplification, surround, emulation, FastTracker 2 panning,
//! end behaviour, resampling quality and the named presets.
//!
//! Every change is applied through [`Command::SetTrackerSettings`](emusic_ui::state::Command::SetTrackerSettings),
//! which the shell pushes to the player live and persists, exactly as the egui
//! page does.

use std::cell::Cell;

use emusic_player::tracker::{
    Emulation, EndBehavior, Interpolation, Ramping, Surround, TrackerSettings,
};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Commands;
use win32ui::prelude::*;
use win32ui::{Button, CheckBox, ComboBox, RadioGroup};

use crate::app::Msg;

use super::super::{FormRow, HEADING_HEIGHT, ROW_HEIGHT, SettingsMsg, labelled, radio_row};

/// Width of a combo-box field in a form row, in design units.
const FIELD_WIDTH: f32 = 220.0;
/// Width of a preset button, in design units.
const PRESET_WIDTH: f32 = 150.0;

/// One field change on [`TrackerSettings`].
pub enum TrackerEdit {
    Interpolation(Interpolation),
    Ramping(Ramping),
    Stereo(u8),
    Amplify(u8),
    Surround(Surround),
    Emulation(Emulation),
    Ft2Pan(bool),
    EndKind(EndKind),
    End(EndBehavior),
    Resampling(u8),
}

/// The named tracker presets, as buttons.
#[derive(Clone, Copy)]
pub enum TrackerPreset {
    BassDefault,
    AmigaAuthentic,
    Smooth,
}

impl TrackerPreset {
    /// The settings a preset applies.
    fn settings(self) -> TrackerSettings {
        match self {
            Self::BassDefault => TrackerSettings::default_preset(),
            Self::AmigaAuthentic => TrackerSettings::amiga_authentic(),
            Self::Smooth => TrackerSettings::smooth(),
        }
    }
}

/// The three end-of-module behaviours, as a combo-box value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EndKind {
    StopAtEnd,
    FollowLoops,
    LoopTimes,
}

impl EndKind {
    fn from_behavior(end: EndBehavior) -> Self {
        match end {
            EndBehavior::StopAtEnd => Self::StopAtEnd,
            EndBehavior::FollowLoops => Self::FollowLoops,
            EndBehavior::LoopTimes(_) => Self::LoopTimes,
        }
    }
}

/// The tracker settings section's controls.
pub(super) struct TrackerSection {
    heading: Label,
    interp_label: Label,
    interpolation: RadioGroup<Interpolation, Msg>,
    ramping_label: Label,
    ramping: ComboBox<Ramping, Msg>,
    surround_label: Label,
    surround: ComboBox<Surround, Msg>,
    emulation_label: Label,
    emulation: ComboBox<Emulation, Msg>,
    stereo_label: Label,
    stereo: Slider<Msg>,
    amplify_label: Label,
    amplify: Slider<Msg>,
    resampling_label: Label,
    resampling: Slider<Msg>,
    ft2: CheckBox<Msg>,
    end_label: Label,
    end: ComboBox<EndKind, Msg>,
    times_label: Label,
    times: Slider<Msg>,
    presets_label: Label,
    presets: [Button<Msg>; 3],
    /// The last loop count shown/taken, so switching to "Loop times" keeps it.
    loop_times: Cell<u8>,
}

impl TrackerSection {
    /// Builds the section's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let interpolation = RadioGroup::new(
            ui,
            [
                ("None", Interpolation::None),
                ("Linear", Interpolation::Linear),
                ("Sinc", Interpolation::Sinc),
            ],
        )?
        .on_select(|value| {
            Some(Msg::Settings(SettingsMsg::Tracker(
                TrackerEdit::Interpolation(*value),
            )))
        });

        let ramping = ComboBox::new(
            ui,
            [
                ("Off", Ramping::Off),
                ("Normal", Ramping::Normal),
                ("Sensitive", Ramping::Sensitive),
            ],
        )?
        .on_select(|value| {
            Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Ramping(
                *value,
            ))))
        });

        let surround = ComboBox::new(
            ui,
            [
                ("Off", Surround::Off),
                ("Mode 1", Surround::Mode1),
                ("Mode 2", Surround::Mode2),
            ],
        )?
        .on_select(|value| {
            Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Surround(
                *value,
            ))))
        });

        let emulation = ComboBox::new(
            ui,
            [
                ("Auto", Emulation::Auto),
                ("FastTracker 2", Emulation::Ft2),
                ("ProTracker 1", Emulation::Pt1),
            ],
        )?
        .on_select(|value| {
            Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Emulation(
                *value,
            ))))
        });

        let stereo = Slider::new(ui, 0.0..=100.0)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Stereo(
                value.round() as u8,
            ))))
        });
        let amplify = Slider::new(ui, 0.0..=100.0)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Amplify(
                value.round() as u8,
            ))))
        });
        let resampling = Slider::new(ui, 0.0..=4.0)?
            .tick_marks(5)
            .on_change(|value| {
                Some(Msg::Settings(SettingsMsg::Tracker(
                    TrackerEdit::Resampling(value.round() as u8),
                )))
            });

        let ft2 = CheckBox::new(ui, "FastTracker 2 panning")?
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Ft2Pan(on)))));

        let end = ComboBox::new(
            ui,
            [
                ("Stop at end", EndKind::StopAtEnd),
                ("Follow loops", EndKind::FollowLoops),
                ("Loop times", EndKind::LoopTimes),
            ],
        )?
        .on_select(|kind| {
            Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::EndKind(
                *kind,
            ))))
        });

        let times = Slider::new(ui, 1.0..=99.0)?.on_change(|value| {
            Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::End(
                EndBehavior::LoopTimes(value.round() as u8),
            ))))
        });

        let presets = [
            Button::new(ui, "BASS default")?.on_click(|| {
                Some(Msg::Settings(SettingsMsg::TrackerPreset(
                    TrackerPreset::BassDefault,
                )))
            }),
            Button::new(ui, "Amiga authentic")?.on_click(|| {
                Some(Msg::Settings(SettingsMsg::TrackerPreset(
                    TrackerPreset::AmigaAuthentic,
                )))
            }),
            Button::new(ui, "Smooth")?.on_click(|| {
                Some(Msg::Settings(SettingsMsg::TrackerPreset(
                    TrackerPreset::Smooth,
                )))
            }),
        ];

        Ok(Self {
            heading: Label::new(ui, Rect::default(), "Tracker modules")?,
            interp_label: Label::new(ui, Rect::default(), "Interpolation")?,
            interpolation,
            ramping_label: Label::new(ui, Rect::default(), "Ramping")?,
            ramping,
            surround_label: Label::new(ui, Rect::default(), "Surround")?,
            surround,
            emulation_label: Label::new(ui, Rect::default(), "Emulation")?,
            emulation,
            stereo_label: Label::new(ui, Rect::default(), "Stereo separation")?,
            stereo,
            amplify_label: Label::new(ui, Rect::default(), "Amplify")?,
            amplify,
            resampling_label: Label::new(ui, Rect::default(), "Resampling quality")?,
            resampling,
            ft2,
            end_label: Label::new(ui, Rect::default(), "End behaviour")?,
            end,
            times_label: Label::new(ui, Rect::default(), "Loop times")?,
            times,
            presets_label: Label::new(ui, Rect::default(), "Presets")?,
            presets,
            loop_times: Cell::new(1),
        })
    }

    /// The section's controls as form rows, in display order.
    pub(super) fn rows(&self) -> Vec<FormRow> {
        let mut preset_row = Layout::row().spacing(dip(8.0));
        for button in &self.presets {
            preset_row = preset_row.item(button.width(dip(PRESET_WIDTH)));
        }
        vec![
            (self.heading.height(dip(HEADING_HEIGHT)), HEADING_HEIGHT),
            (
                labelled(
                    &self.interp_label,
                    radio_row(&self.interpolation).height(dip(ROW_HEIGHT)),
                ),
                ROW_HEIGHT,
            ),
            (
                labelled(&self.ramping_label, self.ramping.width(dip(FIELD_WIDTH))),
                ROW_HEIGHT,
            ),
            (
                labelled(&self.surround_label, self.surround.width(dip(FIELD_WIDTH))),
                ROW_HEIGHT,
            ),
            (
                labelled(
                    &self.emulation_label,
                    self.emulation.width(dip(FIELD_WIDTH)),
                ),
                ROW_HEIGHT,
            ),
            (
                labelled(&self.stereo_label, self.stereo.fill(1)),
                ROW_HEIGHT,
            ),
            (
                labelled(&self.amplify_label, self.amplify.fill(1)),
                ROW_HEIGHT,
            ),
            (
                labelled(&self.resampling_label, self.resampling.fill(1)),
                ROW_HEIGHT,
            ),
            (self.ft2.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (
                labelled(&self.end_label, self.end.width(dip(FIELD_WIDTH))),
                ROW_HEIGHT,
            ),
            (labelled(&self.times_label, self.times.fill(1)), ROW_HEIGHT),
            (
                labelled(&self.presets_label, preset_row.height(dip(ROW_HEIGHT))),
                ROW_HEIGHT,
            ),
        ]
    }

    /// Mirrors the shared tracker settings onto the controls.
    pub(super) fn sync(&self, state: &AppState) {
        let settings = state.tracker_settings;
        self.interpolation.set_selected(&settings.interpolation);
        self.ramping.set_selected(&settings.ramping);
        self.surround.set_selected(&settings.surround);
        self.emulation.set_selected(&settings.emulation);
        self.stereo.set_value(f64::from(settings.stereo_separation));
        self.amplify.set_value(f64::from(settings.amplify));
        self.resampling
            .set_value(f64::from(settings.resampling_quality));
        self.ft2.set_checked(settings.ft2_pan);
        self.end.set_selected(&EndKind::from_behavior(settings.end));
        if let EndBehavior::LoopTimes(times) = settings.end {
            self.loop_times.set(times);
            self.times.set_value(f64::from(times));
        }
        self.times
            .set_enabled(matches!(settings.end, EndBehavior::LoopTimes(_)));
    }

    /// Handles the tracker messages; returns whether `msg` was one.
    pub(super) fn update(
        &self,
        msg: &SettingsMsg,
        state: &mut AppState,
        out: &mut Commands,
    ) -> bool {
        match msg {
            SettingsMsg::Tracker(edit) => {
                let mut settings = state.tracker_settings;
                self.apply(edit, &mut settings);
                settings.sanitize();
                out.push(Command::SetTrackerSettings(settings));
            }
            SettingsMsg::TrackerPreset(preset) => {
                out.push(Command::SetTrackerSettings(preset.settings()));
            }
            _ => return false,
        }
        true
    }

    /// Merges one field change into `settings`.
    fn apply(&self, edit: &TrackerEdit, settings: &mut TrackerSettings) {
        match edit {
            TrackerEdit::Interpolation(value) => settings.interpolation = *value,
            TrackerEdit::Ramping(value) => settings.ramping = *value,
            TrackerEdit::Stereo(value) => settings.stereo_separation = *value,
            TrackerEdit::Amplify(value) => settings.amplify = *value,
            TrackerEdit::Surround(value) => settings.surround = *value,
            TrackerEdit::Emulation(value) => settings.emulation = *value,
            TrackerEdit::Ft2Pan(value) => settings.ft2_pan = *value,
            TrackerEdit::EndKind(kind) => {
                settings.end = match kind {
                    EndKind::StopAtEnd => EndBehavior::StopAtEnd,
                    EndKind::FollowLoops => EndBehavior::FollowLoops,
                    EndKind::LoopTimes => EndBehavior::LoopTimes(self.loop_times.get()),
                };
            }
            TrackerEdit::End(end) => {
                if let EndBehavior::LoopTimes(times) = end {
                    self.loop_times.set(*times);
                }
                settings.end = *end;
            }
            TrackerEdit::Resampling(value) => settings.resampling_quality = *value,
        }
    }
}
