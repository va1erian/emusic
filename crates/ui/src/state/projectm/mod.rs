//! projectM visualization state (#295, #300): where it is shown, how it
//! behaves, and the engine status the app reports. Toolkit-agnostic, so
//! the app shares it; the engine itself lives in the app's surface widget.

mod layout;
mod monitor;
mod settings;
#[cfg(test)]
mod tests;

pub use layout::{VizDock, VizLayout, VizSurface};
pub use monitor::{VizMonitor, pick_monitor};
pub use settings::ProjectMSettings;

use std::path::PathBuf;

/// Whether the frontend could start projectM, as last reported by its
/// surface widget. Transient, never persisted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProjectMAvailability {
    /// No surface has tried to start projectM yet.
    #[default]
    Unknown,
    /// projectM runs; carries its version string.
    Available(String),
    /// The projectM libraries could not be loaded.
    MissingLibrary,
    /// No OpenGL 3.3 context could be created.
    NoOpenGl,
}

/// A one-shot preset navigation request, drained by the frontend surface
/// that owns the running engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetRequest {
    Next,
    Previous,
    Random,
    /// Show the preset at this playlist index (the preset browser, #338).
    Index(usize),
}

/// A visualization request, carried by [`Command::Viz`](super::Command::Viz).
#[derive(Debug, Clone, PartialEq)]
pub enum VizCommand {
    /// Show or hide the visualization.
    SetVisible(bool),
    /// Flip between shown and hidden.
    ToggleVisible,
    /// Show it docked at `dock`, leaving fullscreen.
    SetDock(VizDock),
    /// Enter (showing it) or leave fullscreen.
    SetFullscreen(bool),
    /// Choose the fullscreen monitor.
    SetFullscreenMonitor(VizMonitor),
    /// Switch presets on the running engine.
    Preset(PresetRequest),
    /// Lock or unlock the current preset.
    TogglePresetLock,
    /// Replace the engine/preset settings.
    SetSettings(ProjectMSettings),
    /// Reported by the frontend when a preset starts showing.
    PresetShown(PathBuf),
}

/// Everything the shell keeps about the visualization.
#[derive(Debug, Clone, Default)]
pub struct ProjectMState {
    /// Persisted placement.
    pub layout: VizLayout,
    /// Persisted engine/preset settings.
    pub settings: ProjectMSettings,
    /// Engine status reported by the frontend. Transient.
    pub availability: ProjectMAvailability,
    /// Whether a surface is actually rendering right now. The frontend clears
    /// it while the visualization is hidden, collapsed or minimised, so the
    /// shell's frame-rate wake falls back to its idle cadence (#305).
    /// Transient, never persisted.
    pub running: bool,
    /// Preset navigation queued for the frontend. Transient.
    requests: Vec<PresetRequest>,
}

impl ProjectMState {
    /// Applies one [`VizCommand`].
    pub fn apply(&mut self, cmd: &VizCommand) {
        match cmd {
            VizCommand::SetVisible(visible) => self.layout.visible = *visible,
            VizCommand::ToggleVisible => self.layout.visible = !self.layout.visible,
            VizCommand::SetDock(dock) => {
                self.layout.dock = *dock;
                self.layout.fullscreen = false;
                self.layout.visible = true;
            }
            VizCommand::SetFullscreen(on) => {
                self.layout.fullscreen = *on;
                if *on {
                    self.layout.visible = true;
                }
            }
            VizCommand::SetFullscreenMonitor(monitor) => {
                self.layout.fullscreen_monitor = Some(monitor.clone());
            }
            VizCommand::Preset(request) => {
                // Only a shown engine can act on it; a stale request must
                // not fire when the visualization is shown again later.
                if self.layout.visible {
                    self.requests.push(*request);
                }
            }
            VizCommand::TogglePresetLock => {
                self.settings.preset_locked = !self.settings.preset_locked;
            }
            VizCommand::SetSettings(settings) => self.settings = settings.sanitized(),
            VizCommand::PresetShown(path) => self.settings.last_preset = Some(path.clone()),
        }
        if !self.layout.visible {
            self.requests.clear();
        }
    }

    /// Takes the preset requests queued since the last call, oldest first.
    pub fn take_requests(&mut self) -> Vec<PresetRequest> {
        std::mem::take(&mut self.requests)
    }

    /// The surface to draw on, or `None` while hidden.
    pub fn surface(&self) -> Option<VizSurface> {
        self.layout.surface()
    }
}
