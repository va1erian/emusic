//! [`PlayerApi`] fake backend: a synthetic "now playing" state with a
//! moving position, a small queue, and a synthetic spectrum for the
//! visualizer strip. No audio, no BASS.

use std::time::Duration;

use crate::player_api::{NowPlayingInfo, PlaybackStatus, PlayerApi, QueueEntry, RepeatMode};

const SPECTRUM_BINS: usize = 32;

pub struct MockPlayer {
    status: PlaybackStatus,
    now_playing: Option<NowPlayingInfo>,
    position: Duration,
    volume: f32,
    repeat: RepeatMode,
    shuffle: bool,
    queue: Vec<QueueEntry>,
    spectrum: [f32; SPECTRUM_BINS],
    /// Monotonically increasing phase used to animate the fake spectrum
    /// deterministically (no wall-clock reads).
    phase: f32,
}

impl MockPlayer {
    /// A player pre-loaded with a track and playing, useful for
    /// screenshots of the now-playing panel/status bar.
    pub fn playing_demo() -> Self {
        Self {
            now_playing: Some(NowPlayingInfo {
                title: "Neon Horizon".to_string(),
                artist: "Crimson Wolves".to_string(),
                album: "Static Echo".to_string(),
                duration: Duration::from_secs(214),
            }),
            status: PlaybackStatus::Playing,
            position: Duration::from_secs(76),
            queue: vec![
                QueueEntry {
                    title: "Iron Garden".to_string(),
                    artist: "Silent Machine".to_string(),
                },
                QueueEntry {
                    title: "Golden Tides".to_string(),
                    artist: "Broken Signal".to_string(),
                },
                QueueEntry {
                    title: "夜の街 - Lantern".to_string(),
                    artist: "桜 Bloom".to_string(),
                },
            ],
            ..Self::default()
        }
    }
}

impl Default for MockPlayer {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            now_playing: None,
            position: Duration::ZERO,
            volume: 0.8,
            repeat: RepeatMode::Off,
            shuffle: false,
            queue: Vec::new(),
            spectrum: [0.0; SPECTRUM_BINS],
            phase: 0.0,
        }
    }
}

impl PlayerApi for MockPlayer {
    fn tick(&mut self, dt: Duration) {
        if self.status != PlaybackStatus::Playing {
            return;
        }
        if let Some(np) = &self.now_playing {
            self.position = (self.position + dt).min(np.duration);
        }
        self.phase += dt.as_secs_f32();
        for (i, bin) in self.spectrum.iter_mut().enumerate() {
            let f = i as f32 / SPECTRUM_BINS as f32;
            *bin = (0.5 + 0.5 * (self.phase * (2.0 + f * 5.0) + f * 10.0).sin()).clamp(0.0, 1.0);
        }
    }

    fn status(&self) -> PlaybackStatus {
        self.status
    }

    fn now_playing(&self) -> Option<&NowPlayingInfo> {
        self.now_playing.as_ref()
    }

    fn position(&self) -> Duration {
        self.position
    }

    fn duration(&self) -> Option<Duration> {
        self.now_playing.as_ref().map(|np| np.duration)
    }

    fn volume(&self) -> f32 {
        self.volume
    }

    fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    fn shuffle(&self) -> bool {
        self.shuffle
    }

    fn queue(&self) -> &[QueueEntry] {
        &self.queue
    }

    fn spectrum(&self) -> &[f32] {
        &self.spectrum
    }

    fn play_pause(&mut self) {
        self.status = match self.status {
            PlaybackStatus::Playing => PlaybackStatus::Paused,
            PlaybackStatus::Paused | PlaybackStatus::Stopped => PlaybackStatus::Playing,
        };
    }

    fn stop(&mut self) {
        self.status = PlaybackStatus::Stopped;
        self.position = Duration::ZERO;
    }

    fn next(&mut self) {
        if !self.queue.is_empty() {
            let entry = self.queue.remove(0);
            if let Some(np) = &mut self.now_playing {
                np.title = entry.title;
                np.artist = entry.artist;
            }
            self.position = Duration::ZERO;
        }
    }

    fn previous(&mut self) {
        self.position = Duration::ZERO;
    }

    fn seek(&mut self, position: Duration) {
        self.position = self
            .duration()
            .map_or(position, |total| position.min(total));
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle = enabled;
    }
}
