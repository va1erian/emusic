//! [`PlayerApi`] fake backend: a synthetic "now playing" state with a
//! moving position, a small queue, synthetic module info for tracker
//! formats, and a synthetic spectrum for the visualizer strip. No audio,
//! no BASS.

use std::path::{Path, PathBuf};
use std::time::Duration;

use emusic_player::{
    ExplicitQueueSnapshot, QueueSnapshot, RepeatMode as PlayerRepeatMode, ShuffleSnapshot,
};

use crate::library_api::TrackInfo;
use crate::player_api::{
    ModuleInfo, NowPlayingInfo, PlaybackStatus, PlayerApi, QueueEntry, RepeatMode,
};

/// Derives a display title from a file path (its file stem), for the mock
/// player's fake metadata-free "now playing"/queue entries created by
/// [`PlayerApi::replace_and_play`]/`play_next`/`enqueue` (#11).
fn label(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string()
}

/// A queue panel entry for `path` (metadata-free, like the real adapter).
fn queue_entry(path: &Path) -> QueueEntry {
    QueueEntry {
        title: label(path),
        artist: String::new(),
    }
}

/// Maps the shell's [`RepeatMode`] onto the player crate's.
fn to_player_repeat(mode: RepeatMode) -> PlayerRepeatMode {
    match mode {
        RepeatMode::Off => PlayerRepeatMode::Off,
        RepeatMode::All => PlayerRepeatMode::All,
        RepeatMode::One => PlayerRepeatMode::One,
    }
}

const SPECTRUM_BINS: usize = 512;
const SCOPE_SAMPLES: usize = 512;
const TRACKER_FORMATS: &[&str] = &["xm", "it", "mod", "s3m"];
/// Number of upcoming entries the mock materialises for a shuffle scope.
const SHUFFLE_PREVIEW: usize = 20;

pub struct MockPlayer {
    status: PlaybackStatus,
    now_playing: Option<NowPlayingInfo>,
    position: Duration,
    volume: f32,
    repeat: RepeatMode,
    shuffle: bool,
    queue: Vec<QueueEntry>,
    /// Upcoming track paths, parallel to `queue`, so a queue snapshot can
    /// round-trip them (#214). Display-only demo entries have no path here.
    queue_paths: Vec<PathBuf>,
    module_info: Option<ModuleInfo>,
    spectrum: [f32; SPECTRUM_BINS],
    samples: [f32; SCOPE_SAMPLES],
    /// Monotonically increasing phase used to animate the fake spectrum
    /// deterministically (no wall-clock reads).
    phase: f32,
    /// Label of the active scoped shuffle, if any (#57).
    shuffle_scope: Option<String>,
    /// Transient status line shown in the status bar.
    status_message: Option<String>,
}

impl MockPlayer {
    /// A player pre-loaded with `track` and playing, useful for screenshots
    /// of the now-playing panel/status bar and, importantly, of the track
    /// table's playing-row highlight: passing an actual track from
    /// [`super::MockLibrary`] (rather than made-up metadata) means the
    /// title/artist match a real row so the highlight actually shows up.
    pub fn playing_demo(track: &TrackInfo) -> Self {
        Self {
            now_playing: Some(NowPlayingInfo {
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                path: track.path.clone(),
                duration: track.duration,
            }),
            status: PlaybackStatus::Playing,
            position: Duration::from_secs(76).min(track.duration),
            module_info: tracker_module_info(&track.path, &track.title, Duration::ZERO),
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

    /// Loads `path` as the current track at `position`, playing when `play`
    /// is true and paused otherwise (mirrors the real adapter's session
    /// restore, #214).
    fn load_current(&mut self, path: &Path, position: Duration, play: bool) {
        let title = label(path);
        let path = path.to_string_lossy().into_owned();
        self.now_playing = Some(NowPlayingInfo {
            title: title.clone(),
            artist: String::new(),
            album: String::new(),
            path: path.clone(),
            duration: Duration::ZERO,
        });
        self.status = if play {
            PlaybackStatus::Playing
        } else {
            PlaybackStatus::Paused
        };
        self.position = position;
        self.module_info = tracker_module_info(&path, &title, position);
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
            queue_paths: Vec::new(),
            module_info: None,
            spectrum: [0.0; SPECTRUM_BINS],
            samples: [0.0; SCOPE_SAMPLES],
            phase: 0.0,
            shuffle_scope: None,
            status_message: None,
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
        // A pink-noise-ish, bass-heavy spectrum with a few moving partials,
        // so the visualizer's log-spaced bars look like real music.
        for (i, bin) in self.spectrum.iter_mut().enumerate() {
            let f = i as f32 / SPECTRUM_BINS as f32;
            let rolloff = (1.0 - f).powf(1.6);
            let wiggle = 0.5 + 0.5 * (self.phase * (2.0 + f * 5.0) + f * 10.0).sin();
            *bin = (rolloff * (0.35 + 0.65 * wiggle)).clamp(0.0, 1.0);
        }
        for (i, sample) in self.samples.iter_mut().enumerate() {
            let t = i as f32 / SCOPE_SAMPLES as f32;
            *sample = (t * std::f32::consts::TAU * 4.0 + self.phase * 6.0).sin() * 0.6;
        }
        if let Some(np) = &self.now_playing {
            self.module_info = tracker_module_info(&np.path, &np.title, self.position);
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

    fn shuffle_scope(&self) -> Option<&str> {
        self.shuffle_scope.as_deref()
    }

    fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    fn queue(&self) -> &[QueueEntry] {
        &self.queue
    }

    fn module_info(&self) -> Option<&ModuleInfo> {
        self.module_info.as_ref()
    }

    fn fft(&self) -> Vec<f32> {
        self.spectrum.to_vec()
    }

    fn samples(&self) -> Vec<f32> {
        self.samples.to_vec()
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
            let path = if self.queue_paths.is_empty() {
                PathBuf::new()
            } else {
                self.queue_paths.remove(0)
            };
            if let Some(np) = &mut self.now_playing {
                np.title = entry.title;
                np.artist = entry.artist;
                np.path = path.to_string_lossy().into_owned();
            }
            self.position = Duration::ZERO;
            self.module_info = None;
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

    fn set_tracker_settings(&mut self, _settings: &emusic_player::tracker::TrackerSettings) {
        // Nothing to apply: mock playback has no real tracker engine.
    }

    fn set_midi_soundfont(&mut self, _path: Option<&std::path::Path>) {}

    fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle = enabled;
        if !enabled {
            self.shuffle_scope = None;
        }
    }

    /// Mock playback is always seekable; the unknown-duration/disabled-slider
    /// paths are exercised by the player crate's mock-backend tests (#192).
    fn seek_supported(&self) -> bool {
        true
    }

    fn set_songlengths_path(&mut self, _path: Option<&Path>) {}

    fn set_sid_fallback_length(&mut self, _length: Duration) {}

    fn queue_jump(&mut self, index: usize) {
        if index >= self.queue.len() {
            return;
        }
        for _ in 0..index {
            self.queue.remove(0);
            if !self.queue_paths.is_empty() {
                self.queue_paths.remove(0);
            }
        }
        let entry = self.queue.remove(0);
        let path = if self.queue_paths.is_empty() {
            PathBuf::new()
        } else {
            self.queue_paths.remove(0)
        };
        if let Some(np) = &mut self.now_playing {
            np.title = entry.title;
            np.artist = entry.artist;
            np.path = path.to_string_lossy().into_owned();
        }
        self.position = Duration::ZERO;
        self.module_info = None;
    }

    fn queue_remove(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);
            if index < self.queue_paths.len() {
                self.queue_paths.remove(index);
            }
        }
    }

    fn replace_and_play(&mut self, paths: &[PathBuf], start_index: usize) {
        let Some(start) = paths.get(start_index) else {
            return;
        };
        self.load_current(start, Duration::ZERO, true);
        self.queue_paths = paths[start_index.saturating_add(1)..].to_vec();
        self.queue = self.queue_paths.iter().map(|p| queue_entry(p)).collect();
        self.shuffle_scope = None;
    }

    fn queue_snapshot(&self) -> QueueSnapshot {
        let mut items: Vec<PathBuf> = Vec::new();
        if let Some(np) = &self.now_playing {
            items.push(PathBuf::from(&np.path));
        }
        let current_len = items.len();
        items.extend(self.queue_paths.iter().cloned());
        let total = items.len();
        let repeat = to_player_repeat(self.repeat);
        if let Some(scope) = &self.shuffle_scope {
            QueueSnapshot::Shuffle(ShuffleSnapshot {
                items,
                label: scope.clone(),
                history: if current_len == 0 {
                    Vec::new()
                } else {
                    vec![0]
                },
                cursor: 0,
                bag: (current_len..total).collect(),
                repeat,
            })
        } else {
            QueueSnapshot::Explicit(ExplicitQueueSnapshot {
                order: (0..items.len()).collect(),
                pos: (current_len > 0).then_some(0),
                items,
                shuffle: self.shuffle,
                repeat,
            })
        }
    }

    fn restore_queue(&mut self, snapshot: &QueueSnapshot, position: Duration, play: bool) {
        let (items, pos) = match snapshot {
            QueueSnapshot::Explicit(snapshot) => {
                (snapshot.items.clone(), snapshot.pos.unwrap_or(0))
            }
            QueueSnapshot::Shuffle(snapshot) => (snapshot.items.clone(), 0),
        };
        let Some(current) = items.get(pos) else {
            self.now_playing = None;
            self.queue.clear();
            self.queue_paths.clear();
            self.status = PlaybackStatus::Stopped;
            self.shuffle_scope = None;
            return;
        };
        self.load_current(current, position, play);
        self.queue_paths = items[pos.saturating_add(1)..].to_vec();
        self.queue = self.queue_paths.iter().map(|p| queue_entry(p)).collect();
        match snapshot {
            QueueSnapshot::Shuffle(snapshot) => {
                self.shuffle = true;
                self.shuffle_scope = Some(snapshot.label.clone());
            }
            QueueSnapshot::Explicit(snapshot) => {
                self.shuffle = snapshot.shuffle;
                self.shuffle_scope = None;
            }
        }
        self.status_message = None;
    }

    fn play_next(&mut self, path: &Path) {
        self.queue.insert(0, queue_entry(path));
        self.queue_paths.insert(0, path.to_path_buf());
    }

    fn play_shuffled(&mut self, paths: &[PathBuf], scope_label: &str) {
        let Some(first) = paths.first() else {
            return;
        };
        self.load_current(first, Duration::ZERO, true);
        self.queue_paths = paths
            .iter()
            .skip(1)
            .take(SHUFFLE_PREVIEW)
            .cloned()
            .collect();
        self.queue = self.queue_paths.iter().map(|p| queue_entry(p)).collect();
        self.shuffle = true;
        self.shuffle_scope = Some(scope_label.to_string());
        self.status_message = None;
    }

    fn enqueue(&mut self, path: &Path) {
        self.queue.push(queue_entry(path));
        self.queue_paths.push(path.to_path_buf());
    }
}

/// Build deterministic fake tracker-module metadata when the current path
/// ends in a tracker extension.
fn tracker_module_info(path: &str, title: &str, position: Duration) -> Option<ModuleInfo> {
    let ext = path.rsplit('.').next()?;
    if !TRACKER_FORMATS.contains(&ext.to_ascii_lowercase().as_str()) {
        return None;
    }

    let format_name = match ext.to_ascii_lowercase().as_str() {
        "xm" => "FastTracker II",
        "it" => "Impulse Tracker",
        "mod" => "ProTracker",
        "s3m" => "Scream Tracker 3",
        _ => "Tracker module",
    }
    .to_string();

    let orders = 32u32;
    let rows_per_order = 64u32;
    let total_rows = orders * rows_per_order;
    let elapsed_rows = ((position.as_secs_f32() / 0.05) as u32).min(total_rows.saturating_sub(1));
    let current_order = elapsed_rows / rows_per_order;
    let current_row = elapsed_rows % rows_per_order;

    Some(ModuleInfo {
        name: title.to_string(),
        format: format_name,
        channels: 8,
        orders,
        current_order,
        current_row,
        message: "mock module message".to_string(),
        instruments: vec![
            "lead synth".to_string(),
            "amiga bass".to_string(),
            "fm organ".to_string(),
            "chip lead".to_string(),
        ],
        samples: vec![
            "kick drum".to_string(),
            "snare".to_string(),
            "hihat".to_string(),
            "bass".to_string(),
            "pad".to_string(),
        ],
    })
}
