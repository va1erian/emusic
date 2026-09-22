//! Listen-time bookkeeping: how long the current track has actually been
//! audible, fed into the `PlayFinished` accounting event (see
//! [`super::loading`]).

use std::time::Instant;

use crate::events::PlaybackState;

use super::Player;

impl Player {
    /// Adds wall-clock time since the last tick to listen accounting, but
    /// only while actually playing.
    pub(super) fn accrue_listened_time(&mut self) {
        let now = Instant::now();
        if self.state == PlaybackState::Playing {
            if let Some(last) = self.last_tick {
                self.listen.add(now.duration_since(last));
            }
            self.last_tick = Some(now);
        } else {
            self.last_tick = None;
        }
    }
}
