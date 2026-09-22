//! Tracker-module setting application on the current [`Player`] channel.

use crate::events::PlayerEvent;
use crate::tracker::TrackerSettings;

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
}
