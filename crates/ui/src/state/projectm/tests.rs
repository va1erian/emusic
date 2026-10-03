use std::path::PathBuf;

use super::*;

#[test]
fn hidden_by_default_and_has_no_surface() {
    let state = ProjectMState::default();
    assert!(!state.layout.visible);
    assert_eq!(state.surface(), None);
}

#[test]
fn surface_follows_the_dock() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::SetVisible(true));
    assert_eq!(state.surface(), Some(VizSurface::Panel));

    state.apply(&VizCommand::SetDock(VizDock::Window));
    assert_eq!(state.surface(), Some(VizSurface::Window));

    state.apply(&VizCommand::SetDock(VizDock::Panel));
    assert_eq!(state.surface(), Some(VizSurface::Panel));
}

#[test]
fn docking_shows_it() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::SetDock(VizDock::Window));
    assert_eq!(state.surface(), Some(VizSurface::Window));

    state.apply(&VizCommand::SetVisible(false));
    state.apply(&VizCommand::SetDock(VizDock::Panel));
    assert_eq!(state.surface(), Some(VizSurface::Panel));
}

#[test]
fn redocking_where_it_already_is_keeps_it_there() {
    let mut state = ProjectMState::default();
    state.apply(&VizCommand::SetDock(VizDock::Window));
    state.apply(&VizCommand::SetDock(VizDock::Window));
    assert_eq!(state.surface(), Some(VizSurface::Window));
}

#[test]
fn layout_loads_an_old_fullscreen_config_as_window_mode() {
    let layout: VizLayout = toml::from_str(
        r#"
            visible = true
            dock = "panel"
            fullscreen = true

            [fullscreen_monitor]
            device = '\\.\DISPLAY2'
            name = "DELL U2720Q"
        "#,
    )
    .expect("an old layout still parses");
    assert_eq!(
        layout,
        VizLayout {
            visible: true,
            dock: VizDock::Window,
        }
    );
}

#[test]
fn layout_keeps_the_dock_when_it_was_not_fullscreen() {
    let layout: VizLayout =
        toml::from_str("visible = false\ndock = \"panel\"\nfullscreen = false\n").expect("parses");
    assert_eq!(layout, VizLayout::default());
    let layout: VizLayout = toml::from_str("dock = \"window\"").expect("parses");
    assert_eq!(layout.dock, VizDock::Window);
}

#[test]
fn layout_round_trips_without_fullscreen_keys() {
    let layout = VizLayout {
        visible: true,
        dock: VizDock::Window,
    };
    let text = toml::to_string(&layout).expect("serializes");
    assert!(!text.contains("fullscreen"), "{text}");
    assert_eq!(toml::from_str::<VizLayout>(&text).expect("parses"), layout);
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
