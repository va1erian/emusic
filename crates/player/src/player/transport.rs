//! Direct transport control: play/pause/stop/next/previous/seek/volume.

use std::time::{Duration, Instant};

use crate::events::{PlaybackState, PlayerEvent};
use crate::volume::perceptual_to_gain;

use super::Player;

/// A manual "previous" skips to the actual previous track only if playback
/// is within this long of the start; otherwise it restarts the current one.
const PREVIOUS_RESTART_THRESHOLD: Duration = Duration::from_secs(3);

impl Player {
    /// Toggles play/pause. No-op if nothing is loaded.
    pub fn play_pause(&mut self) {
        let Some(current) = &self.current else {
            return;
        };
        let result = match self.state {
            PlaybackState::Playing => current.channel.pause(),
            PlaybackState::Paused | PlaybackState::Stopped => current.channel.play(false),
        };
        if let Err(error) = result {
            self.emit(PlayerEvent::Error(error));
            return;
        }
        match self.state {
            PlaybackState::Playing => {
                self.last_tick = None;
                self.set_state(PlaybackState::Paused);
            }
            PlaybackState::Paused | PlaybackState::Stopped => {
                self.last_tick = Some(Instant::now());
                self.set_state(PlaybackState::Playing);
            }
        }
    }

    /// Stops playback and resets position to the start, keeping the track
    /// loaded (a subsequent `play_pause` resumes it from zero).
    pub fn stop(&mut self) {
        if let Some(current) = &self.current
            && let Err(error) = current.channel.stop()
        {
            self.emit(PlayerEvent::Error(error));
        }
        self.last_tick = None;
        self.set_state(PlaybackState::Stopped);
    }

    /// Skips to the next track per the queue's repeat mode.
    pub fn next(&mut self) {
        self.drop_current_and_account();
        let next = self.queue.advance();
        self.open_current_or_stop(next);
    }

    /// Skips to the previous track, or restarts the current one if more
    /// than [`PREVIOUS_RESTART_THRESHOLD`] has played.
    pub fn previous(&mut self) {
        if let Some(current) = &self.current
            && current.channel.position().unwrap_or_default() > PREVIOUS_RESTART_THRESHOLD
        {
            if let Err(error) = current.channel.seek(Duration::ZERO) {
                self.emit(PlayerEvent::Error(error));
            }
            return;
        }
        self.drop_current_and_account();
        let previous = self.queue.retreat();
        self.open_current_or_stop(previous);
    }

    /// Seeks the current track to `position`.
    pub fn seek(&mut self, position: Duration) {
        if let Some(current) = &self.current
            && let Err(error) = current.channel.seek(position)
        {
            self.emit(PlayerEvent::Error(error));
        }
    }

    /// Sets linear volume (`0.0..=1.0`); the perceptual curve is applied
    /// internally before reaching the backend.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(current) = &self.current
            && let Err(error) = current.channel.set_volume(perceptual_to_gain(self.volume))
        {
            self.emit(PlayerEvent::Error(error));
        }
    }
}
