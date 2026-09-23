//! egui widget for editing tracker module playback settings.

use eframe::egui;
use emusic_player::tracker::{
    Emulation, EndBehavior, Interpolation, Ramping, Surround, TrackerSettings,
};

use crate::state::{AppState, Command};

/// Settings → Tracker playback tab: edits [`AppState::tracker_settings`] and
/// pushes [`Command::SetTrackerSettings`] when it changes, so the shell
/// applies it live to the player and persists it with the rest of the
/// config.
pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let mut settings = state.tracker_settings;
    if tracker_settings_ui(ui, &mut settings) {
        state.push(Command::SetTrackerSettings(settings));
    }

    ui.add_space(12.0);
    ui.label(egui::RichText::new("Presets").weak());
    ui.horizontal_wrapped(|ui| {
        if ui.button("BASS default").clicked() {
            state.push(Command::SetTrackerSettings(
                TrackerSettings::default_preset(),
            ));
        }
        if ui.button("Amiga authentic").clicked() {
            state.push(Command::SetTrackerSettings(
                TrackerSettings::amiga_authentic(),
            ));
        }
        if ui.button("Smooth").clicked() {
            state.push(Command::SetTrackerSettings(TrackerSettings::smooth()));
        }
    });
}

/// Draws a panel of controls for `settings` and returns whether any value
/// changed this frame.
fn tracker_settings_ui(ui: &mut egui::Ui, settings: &mut TrackerSettings) -> bool {
    settings.sanitize();

    let mut changed = false;

    ui.label("Interpolation");
    ui.horizontal(|ui| {
        changed |= ui
            .selectable_value(&mut settings.interpolation, Interpolation::None, "None")
            .changed();
        changed |= ui
            .selectable_value(&mut settings.interpolation, Interpolation::Linear, "Linear")
            .changed();
        changed |= ui
            .selectable_value(&mut settings.interpolation, Interpolation::Sinc, "Sinc")
            .changed();
    });

    changed |= egui::ComboBox::from_label("Ramping")
        .selected_text(ramping_label(settings.ramping))
        .show_ui(ui, |ui| {
            let mut inner = false;
            inner |= ui
                .selectable_value(&mut settings.ramping, Ramping::Off, "Off")
                .changed();
            inner |= ui
                .selectable_value(&mut settings.ramping, Ramping::Normal, "Normal")
                .changed();
            inner |= ui
                .selectable_value(&mut settings.ramping, Ramping::Sensitive, "Sensitive")
                .changed();
            inner
        })
        .inner
        .unwrap_or(false);

    changed |= ui
        .add(egui::Slider::new(&mut settings.stereo_separation, 0..=100).text("stereo separation"))
        .changed();

    changed |= ui
        .add(egui::Slider::new(&mut settings.amplify, 0..=100).text("amplify"))
        .changed();

    changed |= egui::ComboBox::from_label("Surround")
        .selected_text(surround_label(settings.surround))
        .show_ui(ui, |ui| {
            let mut inner = false;
            inner |= ui
                .selectable_value(&mut settings.surround, Surround::Off, "Off")
                .changed();
            inner |= ui
                .selectable_value(&mut settings.surround, Surround::Mode1, "Mode 1")
                .changed();
            inner |= ui
                .selectable_value(&mut settings.surround, Surround::Mode2, "Mode 2")
                .changed();
            inner
        })
        .inner
        .unwrap_or(false);

    changed |= egui::ComboBox::from_label("Emulation")
        .selected_text(emulation_label(settings.emulation))
        .show_ui(ui, |ui| {
            let mut inner = false;
            inner |= ui
                .selectable_value(&mut settings.emulation, Emulation::Auto, "Auto")
                .changed();
            inner |= ui
                .selectable_value(&mut settings.emulation, Emulation::Ft2, "FastTracker 2")
                .changed();
            inner |= ui
                .selectable_value(&mut settings.emulation, Emulation::Pt1, "ProTracker 1")
                .changed();
            inner
        })
        .inner
        .unwrap_or(false);

    changed |= ui
        .checkbox(&mut settings.ft2_pan, "FastTracker 2 panning")
        .changed();

    changed |= end_behavior_ui(ui, &mut settings.end);

    changed |= ui
        .add(egui::Slider::new(&mut settings.resampling_quality, 0..=4).text("resampling quality"))
        .changed();

    changed
}

fn ramping_label(ramping: Ramping) -> &'static str {
    match ramping {
        Ramping::Off => "Off",
        Ramping::Normal => "Normal",
        Ramping::Sensitive => "Sensitive",
    }
}

fn surround_label(surround: Surround) -> &'static str {
    match surround {
        Surround::Off => "Off",
        Surround::Mode1 => "Mode 1",
        Surround::Mode2 => "Mode 2",
    }
}

fn emulation_label(emulation: Emulation) -> &'static str {
    match emulation {
        Emulation::Auto => "Auto",
        Emulation::Ft2 => "FastTracker 2",
        Emulation::Pt1 => "ProTracker 1",
    }
}

fn end_behavior_ui(ui: &mut egui::Ui, end: &mut EndBehavior) -> bool {
    let mut selected = match end {
        EndBehavior::StopAtEnd => 0,
        EndBehavior::FollowLoops => 1,
        EndBehavior::LoopTimes(_) => 2,
    };

    let mut changed = egui::ComboBox::from_label("End behaviour")
        .selected_text(end_label(*end))
        .show_ui(ui, |ui| {
            let mut inner = false;
            inner |= ui
                .selectable_value(&mut selected, 0, "Stop at end")
                .changed();
            inner |= ui
                .selectable_value(&mut selected, 1, "Follow loops")
                .changed();
            inner |= ui
                .selectable_value(&mut selected, 2, "Loop times")
                .changed();
            inner
        })
        .inner
        .unwrap_or(false);

    if selected == 2 {
        let mut times = match end {
            EndBehavior::LoopTimes(n) => *n,
            _ => 1,
        };
        changed |= ui
            .add(egui::Slider::new(&mut times, 1..=99).text("times"))
            .changed();
        *end = EndBehavior::LoopTimes(times);
    } else {
        *end = match selected {
            0 => EndBehavior::StopAtEnd,
            1 => EndBehavior::FollowLoops,
            _ => EndBehavior::StopAtEnd,
        };
    }

    changed
}

fn end_label(end: EndBehavior) -> String {
    match end {
        EndBehavior::StopAtEnd => "Stop at end".to_string(),
        EndBehavior::FollowLoops => "Follow loops".to_string(),
        EndBehavior::LoopTimes(n) => format!("Loop {n} times"),
    }
}
