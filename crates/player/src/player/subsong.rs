//! Subsong/subtune selection on the current [`Player`] channel.

use crate::backend::Subsong;
use crate::events::PlayerEvent;

use super::Player;

impl Player {
    /// The current subsong/subtune of the loaded track and how many it has, or
    /// `None` when the format has no such concept (or nothing is loaded).
    pub fn subsong(&self) -> Option<Subsong> {
        self.current.as_ref()?.channel.subsong()
    }

    /// Switches the loaded track to `subsong` (`1`-based), restarting it. A
    /// no-op when nothing is loaded or the format has no subsongs. Errors are
    /// emitted as [`PlayerEvent::Error`], matching the other setters.
    pub fn select_subsong(&mut self, subsong: u16) {
        let Some(current) = self.current.as_mut() else {
            return;
        };
        if let Err(error) = current.channel.select_subsong(subsong) {
            self.emit(PlayerEvent::Error(error));
            return;
        }
        // A switch can change the reported length *and* whether it is known
        // (e.g. the new subtune has no HVSC entry), so refresh the cache the
        // transport and listen accounting read.
        let capabilities = current.channel.capabilities();
        let duration = capabilities
            .duration_known
            .then(|| current.channel.duration().ok())
            .flatten();
        current.capabilities = capabilities;
        current.duration = duration;
    }
}
