//! Tracker-module setting application on the current [`Player`] channel.

use crate::events::PlayerEvent;
use crate::tracker::{ModuleInfo, TrackerSettings};

use super::Player;

impl Player {
    /// Applies `settings` to the current music channel (if any) and updates
    /// the global `BASS_CONFIG_SRC` resampler quality.
    ///
    /// Errors are emitted as [`PlayerEvent::Error`] rather than returned,
    /// matching the other transport/setter methods.
    pub fn apply_tracker_settings(&mut self, settings: &TrackerSettings) {
        if let Err(error) = self
            .backend
            .set_tracker_resampling_quality(settings.resampling_quality)
        {
            self.emit(PlayerEvent::Error(error));
        }
        if let Some(current) = &self.current
            && let Err(error) = current.channel.apply_tracker_settings(settings)
        {
            self.emit(PlayerEvent::Error(error));
        }
    }

    /// Live tracker-module metadata for the currently playing track, if it's
    /// a module (`None` for a plain audio stream or when nothing is
    /// loaded).
    pub fn module_info(&self) -> Option<ModuleInfo> {
        self.current.as_ref()?.channel.module_info()
    }
}
