use std::path::PathBuf;

use super::*;

fn monitor(device: &str, name: &str) -> VizMonitor {
    VizMonitor {
        device: device.into(),
        name: name.into(),
    }
}

#[test]
fn hidden_by_default_and_has_no_surface() {
    let state = ProjectMState::default();
    assert!(!state.layout.visible);
    assert_eq!(state.surface(), None);
}

#[test]
fn surface_follows_dock_and_fullscreen() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::SetVisible(true));
    assert_eq!(state.surface(), Some(VizSurface::Panel));

    state.apply(&VizCommand::SetDock(VizDock::Window));
    assert_eq!(state.surface(), Some(VizSurface::Window));

    state.apply(&VizCommand::SetFullscreen(true));
    assert_eq!(state.surface(), Some(VizSurface::Fullscreen));

    state.apply(&VizCommand::SetFullscreen(false));
    assert_eq!(
        state.surface(),
        Some(VizSurface::Window),
        "back to its dock"
    );
}

#[test]
fn entering_fullscreen_or_docking_shows_it() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::SetFullscreen(true));
    assert_eq!(state.surface(), Some(VizSurface::Fullscreen));

    state.apply(&VizCommand::SetVisible(false));
    state.apply(&VizCommand::SetDock(VizDock::Panel));
    assert_eq!(state.surface(), Some(VizSurface::Panel));
    assert!(!state.layout.fullscreen);
}

#[test]
fn hiding_keeps_the_placement() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::SetDock(VizDock::Window));
    state.apply(&VizCommand::ToggleVisible);
    assert_eq!(state.surface(), None);
    state.apply(&VizCommand::ToggleVisible);
    assert_eq!(state.surface(), Some(VizSurface::Window));
}

#[test]
fn preset_requests_queue_only_while_shown() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::Preset(PresetRequest::Next));
    assert!(state.take_requests().is_empty());

    state.apply(&VizCommand::SetVisible(true));
    state.apply(&VizCommand::Preset(PresetRequest::Next));
    state.apply(&VizCommand::Preset(PresetRequest::Random));
    assert_eq!(
        state.take_requests(),
        [PresetRequest::Next, PresetRequest::Random]
    );
    assert!(state.take_requests().is_empty(), "drained");
}

#[test]
fn hiding_drops_pending_preset_requests() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::SetVisible(true));
    state.apply(&VizCommand::Preset(PresetRequest::Previous));
    state.apply(&VizCommand::SetVisible(false));
    assert!(state.take_requests().is_empty());
}

#[test]
fn lock_toggle_and_preset_shown_update_settings() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::TogglePresetLock);
    assert!(state.settings.preset_locked);
    state.apply(&VizCommand::PresetShown(PathBuf::from("a/b.milk")));
    assert_eq!(
        state.settings.last_preset.as_deref(),
        Some(PathBuf::from("a/b.milk").as_path())
    );
}

#[test]
fn set_settings_sanitizes_numbers() {
    let mut state = ProjectMState::default();
    let wild = ProjectMSettings {
        preset_duration_secs: 0.0,
        soft_cut_secs: f32::NAN,
        beat_sensitivity: 99.0,
        fps_cap: 0,
        ..ProjectMSettings::default()
    };
    state.apply(&VizCommand::SetSettings(wild));
    let settings = &state.settings;
    assert_eq!(settings.preset_duration_secs, 1.0);
    assert_eq!(
        settings.soft_cut_secs,
        ProjectMSettings::default().soft_cut_secs
    );
    assert_eq!(settings.beat_sensitivity, 10.0);
    assert_eq!(settings.fps_cap, 10);
}

#[test]
fn packs_are_enabled_unless_disabled() {
    let settings = ProjectMSettings {
        disabled_packs: vec!["projectm-classic".into()],
        ..ProjectMSettings::default()
    };
    assert!(settings.pack_enabled("cream-of-the-crop"));
    assert!(!settings.pack_enabled("projectm-classic"));
}

#[test]
fn pick_monitor_prefers_the_saved_device() {
    let connected = [monitor("D1", "A"), monitor("D2", "B")];
    let saved = monitor("D2", "B");
    assert_eq!(pick_monitor(Some(&saved), &connected, Some(0), 0), Some(1));
}

#[test]
fn pick_monitor_matches_by_name_when_the_device_changed() {
    let connected = [monitor("D1", "A"), monitor("D3", "B")];
    let saved = monitor("D2", "B");
    assert_eq!(pick_monitor(Some(&saved), &connected, Some(0), 0), Some(1));
}

#[test]
fn pick_monitor_falls_back_to_the_window_then_the_primary() {
    let connected = [monitor("D1", "A"), monitor("D2", "B")];
    let gone = monitor("D9", "Z");
    assert_eq!(pick_monitor(Some(&gone), &connected, Some(1), 0), Some(1));
    assert_eq!(pick_monitor(None, &connected, None, 1), Some(1));
    assert_eq!(pick_monitor(None, &connected, Some(7), 5), Some(0));
    assert_eq!(pick_monitor(None, &[], Some(0), 0), None);
}

#[test]
fn pick_monitor_ignores_empty_saved_names() {
    let connected = [monitor("D1", ""), monitor("D2", "B")];
    let saved = monitor("", "");
    assert_eq!(pick_monitor(Some(&saved), &connected, None, 1), Some(1));
}
