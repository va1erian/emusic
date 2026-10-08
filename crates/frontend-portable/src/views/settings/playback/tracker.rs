//! Playback page → tracker module settings: interpolation, ramping, stereo
//! separation, amplification, surround, emulation, FastTracker 2 panning, end
//! behaviour, resampling quality and the named presets.
//!
//! Every change is applied through
//! [`Command::SetTrackerSettings`](emusic_ui::state::Command::SetTrackerSettings),
//! which the shell pushes to the player live and persists.

use std::cell::Cell;

use emusic_player::tracker::{
    Emulation, EndBehavior, Interpolation, Ramping, Surround, TrackerSettings,
};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Commands;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, CheckBox, ComboBox, Label, RadioGroup, Slider};

use super::super::SettingsMsg;
use super::super::form::{FormPage, HEADING_HEIGHT, LABEL_WIDTH, ROW_HEIGHT, RowBuilder};
use crate::app::Msg;
use crate::make::make;
use xui::xui_core::arrange::{button, checkbox, combo_box, label, radio_group, slider};

/// Width of a preset button, in design units.
const PRESET_WIDTH: f32 = 150.0;

/// The interpolation radios, in display order.
const INTERPOLATION: [(&str, Interpolation); 3] = [
    ("None", Interpolation::None),
    ("Linear", Interpolation::Linear),
    ("Sinc", Interpolation::Sinc),
];
/// The ramping choices.
const RAMPING: [(&str, Ramping); 3] = [
    ("Off", Ramping::Off),
    ("Normal", Ramping::Normal),
    ("Sensitive", Ramping::Sensitive),
];
/// The surround modes.
const SURROUND: [(&str, Surround); 3] = [
    ("Off", Surround::Off),
    ("Mode 1", Surround::Mode1),
    ("Mode 2", Surround::Mode2),
];
/// The emulation modes.
const EMULATION: [(&str, Emulation); 3] = [
    ("Auto", Emulation::Auto),
    ("FastTracker 2", Emulation::Ft2),
    ("ProTracker 1", Emulation::Pt1),
];

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

/// The end-behaviour choices.
const END: [(&str, EndKind); 3] = [
    ("Stop at end", EndKind::StopAtEnd),
    ("Follow loops", EndKind::FollowLoops),
    ("Loop times", EndKind::LoopTimes),
];

/// The tracker settings section's controls.
pub(super) struct TrackerSection {
    _heading: Label<Msg>,
    _interp_label: Label<Msg>,
    interpolation: RadioGroup<Msg>,
    _ramping_label: Label<Msg>,
    ramping: ComboBox<Msg>,
    _surround_label: Label<Msg>,
    surround: ComboBox<Msg>,
    _emulation_label: Label<Msg>,
    emulation: ComboBox<Msg>,
    _stereo_label: Label<Msg>,
    stereo: Slider<Msg>,
    _amplify_label: Label<Msg>,
    amplify: Slider<Msg>,
    _resampling_label: Label<Msg>,
    resampling: Slider<Msg>,
    ft2: CheckBox<Msg>,
    _end_label: Label<Msg>,
    end: ComboBox<Msg>,
    _times_label: Label<Msg>,
    times: Slider<Msg>,
    _presets_label: Label<Msg>,
    _presets: [Button<Msg>; 3],
    /// The last loop count shown/taken, so switching to "Loop times" keeps it.
    loop_times: Cell<u8>,
}

impl TrackerSection {
    /// Builds the section's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(form: &FormPage) -> TrackerSection {
        let heading = make(form.ui(), label("Tracker modules")).expect("create tracker heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let interp_row = form.row(ROW_HEIGHT).expect("create interpolation row");
        let interp_label =
            make(interp_row.ui(), label("Interpolation")).expect("create interp label");
        let interpolation = make(interp_row.ui(), radio_group(&names(&INTERPOLATION)))
            .expect("create interpolation radios")
            .on_select(|index| {
                Some(Msg::Settings(SettingsMsg::Tracker(
                    TrackerEdit::Interpolation(INTERPOLATION[index].1),
                )))
            });
        interp_row.fixed(interp_label.id(), LABEL_WIDTH).finish();
        for option in interpolation.ids() {
            form.add_full(option, ROW_HEIGHT);
        }

        let ramping_row = form.row(ROW_HEIGHT).expect("create ramping row");
        let ramping_label = make(ramping_row.ui(), label("Ramping")).expect("create ramping label");
        let ramping = make(ramping_row.ui(), combo_box(&names(&RAMPING)))
            .expect("create ramping combo")
            .on_select(|index| {
                Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Ramping(
                    RAMPING[index].1,
                ))))
            });
        labelled(ramping_row, ramping_label.id(), ramping.id());

        let surround_row = form.row(ROW_HEIGHT).expect("create surround row");
        let surround_label =
            make(surround_row.ui(), label("Surround")).expect("create surround label");
        let surround = make(surround_row.ui(), combo_box(&names(&SURROUND)))
            .expect("create surround combo")
            .on_select(|index| {
                Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Surround(
                    SURROUND[index].1,
                ))))
            });
        labelled(surround_row, surround_label.id(), surround.id());

        let emulation_row = form.row(ROW_HEIGHT).expect("create emulation row");
        let emulation_label =
            make(emulation_row.ui(), label("Emulation")).expect("create emulation label");
        let emulation = make(emulation_row.ui(), combo_box(&names(&EMULATION)))
            .expect("create emulation combo")
            .on_select(|index| {
                Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Emulation(
                    EMULATION[index].1,
                ))))
            });
        labelled(emulation_row, emulation_label.id(), emulation.id());

        let stereo_row = form.row(ROW_HEIGHT).expect("create stereo row");
        let stereo_label =
            make(stereo_row.ui(), label("Stereo separation")).expect("create stereo label");
        let stereo = make(stereo_row.ui(), slider(0.0, 100.0))
            .expect("create stereo slider")
            .on_change(|value| {
                Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Stereo(
                    value.round() as u8,
                ))))
            });
        labelled(stereo_row, stereo_label.id(), stereo.id());

        let amplify_row = form.row(ROW_HEIGHT).expect("create amplify row");
        let amplify_label = make(amplify_row.ui(), label("Amplify")).expect("create amplify label");
        let amplify = make(amplify_row.ui(), slider(0.0, 100.0))
            .expect("create amplify slider")
            .on_change(|value| {
                Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Amplify(
                    value.round() as u8,
                ))))
            });
        labelled(amplify_row, amplify_label.id(), amplify.id());

        let resampling_row = form.row(ROW_HEIGHT).expect("create resampling row");
        let resampling_label =
            make(resampling_row.ui(), label("Resampling quality")).expect("create resample label");
        let resampling = make(resampling_row.ui(), slider(0.0, 4.0))
            .expect("create resampling slider")
            .on_change(|value| {
                Some(Msg::Settings(SettingsMsg::Tracker(
                    TrackerEdit::Resampling(value.round() as u8),
                )))
            });
        labelled(resampling_row, resampling_label.id(), resampling.id());

        let ft2 = make(form.ui(), checkbox("FastTracker 2 panning"))
            .expect("create ft2 checkbox")
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::Ft2Pan(on)))));
        form.add_full(ft2.id(), ROW_HEIGHT);

        let end_row = form.row(ROW_HEIGHT).expect("create end row");
        let end_label = make(end_row.ui(), label("End behaviour")).expect("create end label");
        let end = make(end_row.ui(), combo_box(&names(&END)))
            .expect("create end combo")
            .on_select(|index| {
                Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::EndKind(
                    END[index].1,
                ))))
            });
        labelled(end_row, end_label.id(), end.id());

        let times_row = form.row(ROW_HEIGHT).expect("create loop-times row");
        let times_label =
            make(times_row.ui(), label("Loop times")).expect("create loop-times label");
        let times = make(times_row.ui(), slider(1.0, 99.0))
            .expect("create loop-times slider")
            .on_change(|value| {
                Some(Msg::Settings(SettingsMsg::Tracker(TrackerEdit::End(
                    EndBehavior::LoopTimes(value.round() as u8),
                ))))
            });
        labelled(times_row, times_label.id(), times.id());

        let presets_row = form.row(ROW_HEIGHT).expect("create preset row");
        let presets_label = make(presets_row.ui(), label("Presets")).expect("create presets label");
        let presets = [
            make(presets_row.ui(), button("BASS default"))
                .expect("create bass preset")
                .on_click(|| {
                    Some(Msg::Settings(SettingsMsg::TrackerPreset(
                        TrackerPreset::BassDefault,
                    )))
                }),
            make(presets_row.ui(), button("Amiga authentic"))
                .expect("create amiga preset")
                .on_click(|| {
                    Some(Msg::Settings(SettingsMsg::TrackerPreset(
                        TrackerPreset::AmigaAuthentic,
                    )))
                }),
            make(presets_row.ui(), button("Smooth"))
                .expect("create smooth preset")
                .on_click(|| {
                    Some(Msg::Settings(SettingsMsg::TrackerPreset(
                        TrackerPreset::Smooth,
                    )))
                }),
        ];
        let mut presets_row = presets_row;
        presets_row = presets_row.fixed(presets_label.id(), LABEL_WIDTH);
        for button in &presets {
            presets_row = presets_row.fixed(button.id(), dip(PRESET_WIDTH));
        }
        presets_row.finish();

        TrackerSection {
            _heading: heading,
            _interp_label: interp_label,
            interpolation,
            _ramping_label: ramping_label,
            ramping,
            _surround_label: surround_label,
            surround,
            _emulation_label: emulation_label,
            emulation,
            _stereo_label: stereo_label,
            stereo,
            _amplify_label: amplify_label,
            amplify,
            _resampling_label: resampling_label,
            resampling,
            ft2,
            _end_label: end_label,
            end,
            _times_label: times_label,
            times,
            _presets_label: presets_label,
            _presets: presets,
            loop_times: Cell::new(1),
        }
    }

    /// Mirrors the shared tracker settings onto the controls.
    pub(super) fn sync(&self, state: &AppState) {
        let settings = state.tracker_settings;
        self.interpolation
            .select(index_of(&INTERPOLATION, settings.interpolation));
        self.ramping.select(index_of(&RAMPING, settings.ramping));
        self.surround.select(index_of(&SURROUND, settings.surround));
        self.emulation
            .select(index_of(&EMULATION, settings.emulation));
        self.stereo.set_value(f64::from(settings.stereo_separation));
        self.amplify.set_value(f64::from(settings.amplify));
        self.resampling
            .set_value(f64::from(settings.resampling_quality));
        self.ft2.set_checked(settings.ft2_pan);
        self.end
            .select(index_of(&END, EndKind::from_behavior(settings.end)));
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

/// Finishes a "label | field" row.
fn labelled(
    row: RowBuilder<'_>,
    label: xui::xui_core::backend::WidgetId,
    field: xui::xui_core::backend::WidgetId,
) {
    row.fixed(label, LABEL_WIDTH).fill(field).finish();
}

/// The labels of an option table.
fn names<'a, T>(options: &'a [(&str, T)]) -> Vec<&'a str> {
    options.iter().map(|(name, _)| *name).collect()
}

/// The position of `value` in `options`, or 0 when absent.
fn index_of<T: PartialEq>(options: &[(&str, T)], value: T) -> usize {
    options
        .iter()
        .position(|(_, candidate)| *candidate == value)
        .unwrap_or(0)
}
