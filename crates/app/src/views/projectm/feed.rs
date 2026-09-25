#![forbid(unsafe_code)]

//! Audio feed and frame pacing for the projectM surface (#301).
//!
//! The frontend reads PCM from the player once per shell tick; the widget
//! drains it on its own animation tick, so the two rates can differ without
//! losing samples. While nothing plays, silence is fed instead, so presets
//! keep animating. Repaints are throttled to the configured FPS cap.

use std::time::{Duration, Instant};

use emusic_projectm::Parameters;
use emusic_ui::player_api::{PlaybackStatus, PlayerApi};
use emusic_ui::state::projectm::ProjectMSettings;

/// Silence pushed while nothing plays: one stereo block, matching projectM's
/// working buffer size.
const SILENCE_FRAMES: usize = 2048;

/// Cap on buffered samples, so a stalled paint cannot grow the buffer without
/// bound. At 48 kHz stereo this is about two seconds of audio.
const MAX_PENDING: usize = 48_000 * 2 * 2;

/// Interleaved stereo PCM waiting for the next animation tick.
#[derive(Default)]
pub(crate) struct Feed {
    pending: Vec<f32>,
}

impl Feed {
    /// Reads the player for this frame: its samples while playing, silence
    /// otherwise, so the visualization keeps moving while paused.
    pub(crate) fn feed(&mut self, player: &dyn PlayerApi) {
        if player.status() == PlaybackStatus::Playing {
            self.push(&player.samples());
        } else {
            self.push_silence();
        }
    }

    fn push(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        self.pending.extend_from_slice(samples);
        if self.pending.len() > MAX_PENDING {
            let excess = self.pending.len() - MAX_PENDING;
            self.pending.drain(..excess);
        }
    }

    fn push_silence(&mut self) {
        self.pending
            .extend(std::iter::repeat_n(0.0, SILENCE_FRAMES));
    }

    /// Takes everything buffered since the last call, oldest first.
    pub(crate) fn take(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.pending)
    }
}

/// Rejects repaints that arrive sooner than the configured FPS cap.
pub(crate) struct FramePacer {
    interval: Duration,
    last: Option<Instant>,
}

impl FramePacer {
    pub(crate) fn new(fps: u32) -> Self {
        Self {
            interval: interval_for(fps),
            last: None,
        }
    }

    /// Applies a new FPS cap.
    pub(crate) fn set_fps(&mut self, fps: u32) {
        self.interval = interval_for(fps);
    }

    /// Whether a frame is due at `now`, recording it when so.
    pub(crate) fn due(&mut self, now: Instant) -> bool {
        match self.last {
            Some(last) if now.duration_since(last) < self.interval => false,
            _ => {
                self.last = Some(now);
                true
            }
        }
    }
}

/// The frame interval for `fps`, never zero.
fn interval_for(fps: u32) -> Duration {
    Duration::from_secs_f32(1.0 / fps.max(1) as f32)
}

/// Translates the frontend's persisted settings into projectM's numeric ones.
pub(crate) fn parameters_from(settings: &ProjectMSettings) -> Parameters {
    Parameters {
        preset_duration_secs: settings.preset_duration_secs,
        soft_cut_secs: settings.soft_cut_secs,
        hard_cuts: settings.hard_cuts,
        hard_cut_sensitivity: settings.hard_cut_sensitivity,
        beat_sensitivity: settings.beat_sensitivity,
        shuffle: settings.shuffle,
        preset_locked: settings.preset_locked,
        fps: settings.fps_cap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_returns_buffered_samples_oldest_first() {
        let mut feed = Feed::default();
        feed.push(&[1.0, 2.0]);
        feed.push(&[3.0]);
        assert_eq!(feed.take(), vec![1.0, 2.0, 3.0]);
        assert!(feed.take().is_empty());
    }

    #[test]
    fn buffered_samples_are_capped() {
        let mut feed = Feed::default();
        let block = vec![1.0; MAX_PENDING];
        feed.push(&block);
        feed.push(&[2.0, 3.0]);
        let taken = feed.take();
        assert_eq!(taken.len(), MAX_PENDING);
        assert_eq!(&taken[taken.len() - 2..], &[2.0, 3.0]);
    }

    #[test]
    fn pacer_rejects_frames_inside_the_interval() {
        let mut pacer = FramePacer::new(30);
        let start = Instant::now();
        assert!(pacer.due(start));
        assert!(!pacer.due(start + Duration::from_millis(5)));
        assert!(pacer.due(start + Duration::from_millis(40)));
    }

    #[test]
    fn pacer_never_divides_by_zero() {
        let mut pacer = FramePacer::new(0);
        assert!(pacer.due(Instant::now()));
    }

    #[test]
    fn settings_map_onto_projectm_parameters() {
        let settings = ProjectMSettings {
            preset_duration_secs: 12.0,
            soft_cut_secs: 1.5,
            hard_cuts: true,
            hard_cut_sensitivity: 3.0,
            beat_sensitivity: 2.0,
            shuffle: false,
            preset_locked: true,
            fps_cap: 30,
            ..ProjectMSettings::default()
        };
        let params = parameters_from(&settings);
        assert_eq!(params.preset_duration_secs, 12.0);
        assert_eq!(params.soft_cut_secs, 1.5);
        assert!(params.hard_cuts);
        assert_eq!(params.hard_cut_sensitivity, 3.0);
        assert_eq!(params.beat_sensitivity, 2.0);
        assert!(!params.shuffle);
        assert!(params.preset_locked);
        assert_eq!(params.fps, 30);
    }
}
