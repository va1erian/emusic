//! Adapts [`emusic_player::Player`] (real BASS-backed playback) to the
//! shell's [`PlayerApi`] trait (#11).
//!
//! Per `emusic_player`'s own module docs, every `PlayerApi` getter/command
//! has a same-shaped method on [`emusic_player::Player`]; this is close to a
//! 1:1 forwarding wrapper. The one thing it adds is metadata-free display
//! text (title = file stem) for "now playing"/queue entries, since the real
//! library (`crates/library`) isn't wired into the shell yet — that's later
//! issue's scope, not #11's.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

use emusic_library::stats::PlayRecord;
use emusic_player::tracker::TrackerSettings;
use emusic_player::{PlaybackState, Player, PlayerEvent, RepeatMode as PlayerRepeatMode};

use super::PlayMessage;
use crate::player_api::{
    ModuleInfo, NowPlayingInfo, PlaybackStatus, PlayerApi, QueueEntry, RepeatMode,
};

pub struct PlayerAdapter {
    player: Player,
    now_playing: Option<NowPlayingInfo>,
    /// Cached live tracker-module metadata, refreshed once per
    /// [`PlayerApi::tick`] (see [`PlayerAdapter::refresh_cache`]) so
    /// [`PlayerApi::module_info`] can return a borrow.
    module_info: Option<ModuleInfo>,
    queue: Vec<QueueEntry>,
    /// Parallel to `queue`: each displayed entry's index into the player's
    /// original queue list, so `queue_jump`/`queue_remove` (indices over
    /// the *displayed*, upcoming-only list) can address the right track in
    /// [`Player::jump_to`]/[`Player::remove`], which take original-list
    /// indices.
    queue_item_indices: Vec<usize>,
    /// Channel to the library backend's stats recorder. `None` in mock mode
    /// or when the library backend is unavailable.
    play_record_tx: Option<Sender<PlayMessage>>,
    /// Unix timestamp (seconds, UTC) when the current track started playing.
    current_track_started_at: Option<i64>,
    /// Cached label of the active scoped shuffle (#57).
    shuffle_scope: Option<String>,
    /// Cached transient status line (e.g. a skipped unreadable file).
    status_message: Option<String>,
}

impl PlayerAdapter {
    pub(crate) fn new(player: Player, play_record_tx: Option<Sender<PlayMessage>>) -> Self {
        Self {
            player,
            now_playing: None,
            module_info: None,
            queue: Vec::new(),
            queue_item_indices: Vec::new(),
            play_record_tx,
            current_track_started_at: None,
            shuffle_scope: None,
            status_message: None,
        }
    }

    /// Recomputes the cached "now playing"/queue display data from the
    /// player's current path and queue. Called once per [`PlayerApi::tick`],
    /// since `PlayerApi::now_playing`/`queue` return borrows and can't build
    /// their result on the fly.
    fn refresh(&mut self) {
        let duration = self.player.duration();
        self.now_playing = self
            .player
            .current_path()
            .map(|path| now_playing_info(path, duration));
        let upcoming = self.player.upcoming();
        self.queue_item_indices = upcoming.iter().map(|(index, _)| *index).collect();
        self.queue = upcoming.iter().map(|(_, path)| queue_entry(path)).collect();
        self.shuffle_scope = self.player.shuffle_scope().map(str::to_string);
        self.module_info = self.player.module_info().map(map_module_info);
    }
}

/// Converts `emusic_player`'s [`emusic_player::tracker::ModuleInfo`] to the
/// shell-facing [`ModuleInfo`] (kept as a separate type per this module's own
/// docs: everything here is a same-shaped forwarding wrapper).
fn map_module_info(info: emusic_player::tracker::ModuleInfo) -> ModuleInfo {
    ModuleInfo {
        name: info.name,
        format: info.format,
        channels: info.channels,
        orders: info.orders,
        current_order: info.current_order,
        current_row: info.current_row,
        message: info.message,
        instruments: info.instruments,
        samples: info.samples,
    }
}

fn track_label(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string()
}

fn now_playing_info(path: &Path, duration: Option<Duration>) -> NowPlayingInfo {
    NowPlayingInfo {
        title: track_label(path),
        artist: String::new(),
        album: String::new(),
        path: path.to_string_lossy().into_owned(),
        duration: duration.unwrap_or_default(),
    }
}

fn queue_entry(path: &Path) -> QueueEntry {
    QueueEntry {
        title: track_label(path),
        artist: String::new(),
    }
}

fn map_status(state: PlaybackState) -> PlaybackStatus {
    match state {
        PlaybackState::Stopped => PlaybackStatus::Stopped,
        PlaybackState::Playing => PlaybackStatus::Playing,
        PlaybackState::Paused => PlaybackStatus::Paused,
    }
}

fn map_repeat_from_player(mode: PlayerRepeatMode) -> RepeatMode {
    match mode {
        PlayerRepeatMode::Off => RepeatMode::Off,
        PlayerRepeatMode::All => RepeatMode::All,
        PlayerRepeatMode::One => RepeatMode::One,
    }
}

fn map_repeat_to_player(mode: RepeatMode) -> PlayerRepeatMode {
    match mode {
        RepeatMode::Off => PlayerRepeatMode::Off,
        RepeatMode::All => PlayerRepeatMode::All,
        RepeatMode::One => PlayerRepeatMode::One,
    }
}

impl PlayerAdapter {
    fn handle_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::TrackStarted { path, .. } => {
                let started_at = unix_now();
                self.current_track_started_at = Some(started_at);
                self.status_message = None;
                if let Some(tx) = &self.play_record_tx {
                    let _ = tx.send(PlayMessage::Started { path, started_at });
                }
            }
            PlayerEvent::TrackSkipped { path } => {
                self.status_message =
                    Some(format!("Skipped unreadable file: {}", track_label(&path)));
            }
            PlayerEvent::PlayFinished {
                path,
                listened,
                completed,
            } => {
                self.send_record(path, listened, completed);
            }
            _ => {}
        }
    }

    fn send_record(&self, path: PathBuf, listened: Duration, completed: bool) {
        let Some(tx) = &self.play_record_tx else {
            return;
        };
        let Some(started_at) = self.current_track_started_at else {
            return;
        };
        let record = PlayRecord {
            path,
            started_at,
            listened_ms: listened.as_millis() as u32,
            completed,
        };
        let _ = tx.send(PlayMessage::Finished(record));
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl PlayerApi for PlayerAdapter {
    fn tick(&mut self, _dt: Duration) {
        self.player.tick();
        // Collect first so the borrow of `self.player` ends before handling,
        // which needs `&mut self` for the started-at timestamp.
        let events: Vec<PlayerEvent> = self.player.events().try_iter().collect();
        for event in events {
            self.handle_event(event);
        }
        self.refresh();
    }

    fn status(&self) -> PlaybackStatus {
        map_status(self.player.state())
    }

    fn now_playing(&self) -> Option<&NowPlayingInfo> {
        self.now_playing.as_ref()
    }

    fn position(&self) -> Duration {
        self.player.position().unwrap_or_default()
    }

    fn duration(&self) -> Option<Duration> {
        self.player.duration()
    }

    fn volume(&self) -> f32 {
        self.player.volume()
    }

    fn repeat_mode(&self) -> RepeatMode {
        map_repeat_from_player(self.player.repeat_mode())
    }

    fn shuffle(&self) -> bool {
        self.player.shuffle()
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
        // Only reached while the visualizer is on (#25), so the FFT read
        // happens at most ~30 fps and never when the strip is off.
        self.player.fft().unwrap_or_default()
    }

    fn samples(&self) -> Vec<f32> {
        self.player.samples().unwrap_or_default()
    }

    fn play_pause(&mut self) {
        self.player.play_pause();
    }

    fn stop(&mut self) {
        self.player.stop();
    }

    fn next(&mut self) {
        self.player.next();
    }

    fn previous(&mut self) {
        self.player.previous();
    }

    fn seek(&mut self, position: Duration) {
        self.player.seek(position);
    }

    fn set_volume(&mut self, volume: f32) {
        self.player.set_volume(volume);
    }

    fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.player.set_repeat_mode(map_repeat_to_player(mode));
    }

    fn set_shuffle(&mut self, enabled: bool) {
        self.player.set_shuffle(enabled);
    }

    fn seek_supported(&self) -> bool {
        self.player.seek_supported()
    }

    fn set_songlengths_path(&mut self, path: Option<&Path>) {
        self.player.set_songlengths_path(path);
    }

    fn set_sid_fallback_length(&mut self, length: Duration) {
        self.player.set_sid_fallback_length(length);
    }

    fn set_tracker_settings(&mut self, settings: &TrackerSettings) {
        self.player.apply_tracker_settings(settings);
    }

    fn set_midi_soundfont(&mut self, path: Option<&Path>) {
        self.player.set_midi_soundfont(path);
    }

    fn queue_jump(&mut self, index: usize) {
        if let Some(&item_index) = self.queue_item_indices.get(index) {
            self.player.jump_to(item_index);
        }
    }

    fn queue_remove(&mut self, index: usize) {
        if let Some(&item_index) = self.queue_item_indices.get(index) {
            self.player.remove(item_index);
        }
    }

    fn replace_and_play(&mut self, paths: &[PathBuf], start_index: usize) {
        self.player.replace_and_play(paths.to_vec(), start_index);
    }

    fn restore_track(&mut self, path: &Path, position: Duration, play: bool) {
        self.status_message = None;
        self.player
            .replace_and_play_at(vec![path.to_path_buf()], 0, position, play);
    }

    fn play_shuffled(&mut self, paths: &[PathBuf], label: &str) {
        self.status_message = None;
        self.player.play_shuffled(paths.to_vec(), label);
    }

    fn play_next(&mut self, path: &Path) {
        self.player.play_next(path.to_path_buf());
    }

    fn enqueue(&mut self, path: &Path) {
        self.player.enqueue(path.to_path_buf());
    }
}
