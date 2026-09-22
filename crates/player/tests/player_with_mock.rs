//! Exercises [`emusic_player::Player`]'s transport/queue/event/listen-
//! accounting logic through a mock [`AudioBackend`], with no real BASS
//! device involved.

use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use emusic_player::backend::{AudioBackend, BackendChannel};
use emusic_player::tracker::TrackerSettings;
use emusic_player::{PlaybackState, Player, PlayerError, PlayerEvent};

/// A fake channel: position advances with real wall-clock time (like a real
/// player would), "ends" once it reaches `duration`, and lets the test fire
/// its end-of-track callback directly for deterministic tests.
struct MockChannel {
    duration: Duration,
    started_at: Mutex<Option<Instant>>,
    paused_at: Mutex<Option<Duration>>,
    end_callback: Mutex<Option<Box<dyn Fn() + Send>>>,
    volume: Mutex<f32>,
}

impl MockChannel {
    fn new(duration: Duration) -> Self {
        Self {
            duration,
            started_at: Mutex::new(None),
            paused_at: Mutex::new(Some(Duration::ZERO)),
            end_callback: Mutex::new(None),
            volume: Mutex::new(1.0),
        }
    }
}

impl BackendChannel for MockChannel {
    fn play(&self, restart: bool) -> Result<(), PlayerError> {
        if restart {
            *self.paused_at.lock().unwrap() = Some(Duration::ZERO);
        }
        let resume_from = self.paused_at.lock().unwrap().take().unwrap_or_default();
        *self.started_at.lock().unwrap() = Some(Instant::now() - resume_from);
        Ok(())
    }

    fn pause(&self) -> Result<(), PlayerError> {
        let pos = self.position()?;
        *self.paused_at.lock().unwrap() = Some(pos);
        *self.started_at.lock().unwrap() = None;
        Ok(())
    }

    fn stop(&self) -> Result<(), PlayerError> {
        *self.paused_at.lock().unwrap() = Some(Duration::ZERO);
        *self.started_at.lock().unwrap() = None;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.started_at.lock().unwrap().is_some()
    }

    fn position(&self) -> Result<Duration, PlayerError> {
        if let Some(started) = *self.started_at.lock().unwrap() {
            Ok(started.elapsed().min(self.duration))
        } else {
            Ok(self.paused_at.lock().unwrap().unwrap_or_default())
        }
    }

    fn duration(&self) -> Result<Duration, PlayerError> {
        Ok(self.duration)
    }

    fn seek(&self, position: Duration) -> Result<(), PlayerError> {
        if self.started_at.lock().unwrap().is_some() {
            *self.started_at.lock().unwrap() = Some(Instant::now() - position);
        } else {
            *self.paused_at.lock().unwrap() = Some(position);
        }
        Ok(())
    }

    fn set_volume(&self, gain: f32) -> Result<(), PlayerError> {
        *self.volume.lock().unwrap() = gain;
        Ok(())
    }

    fn apply_tracker_settings(&self, _settings: &TrackerSettings) -> Result<(), PlayerError> {
        Ok(())
    }

    fn on_end(&self, callback: Box<dyn Fn() + Send>) -> Result<Box<dyn Any + Send>, PlayerError> {
        *self.end_callback.lock().unwrap() = Some(callback);
        Ok(Box::new(()))
    }
}

/// Mock backend: "opens" instantly (in-process, no real I/O), optionally
/// failing for specific paths, and hands back a channel a test can also
/// grab a handle to via `opened` to simulate track-end.
#[derive(Clone, Default)]
struct MockBackend {
    duration: Duration,
    fail_paths: Arc<Mutex<Vec<PathBuf>>>,
    last_opened: Arc<Mutex<Option<Arc<MockChannel>>>>,
}

impl MockBackend {
    fn new(duration: Duration) -> Self {
        Self {
            duration,
            ..Default::default()
        }
    }

    fn fail_for(&self, path: impl Into<PathBuf>) {
        self.fail_paths.lock().unwrap().push(path.into());
    }

    /// Fires the end-of-track callback for the most recently opened
    /// channel, simulating BASS's `BASS_SYNC_END`.
    fn end_current_track(&self) {
        let channel = self.last_opened.lock().unwrap().clone();
        if let Some(channel) = channel
            && let Some(cb) = channel.end_callback.lock().unwrap().as_ref()
        {
            cb();
        }
    }
}

/// Wraps an `Arc<MockChannel>` so it can be handed out as a `BackendChannel`
/// trait object while the backend keeps its own strong reference.
struct SharedChannel(Arc<MockChannel>);

impl BackendChannel for SharedChannel {
    fn play(&self, restart: bool) -> Result<(), PlayerError> {
        self.0.play(restart)
    }
    fn pause(&self) -> Result<(), PlayerError> {
        self.0.pause()
    }
    fn stop(&self) -> Result<(), PlayerError> {
        self.0.stop()
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn position(&self) -> Result<Duration, PlayerError> {
        self.0.position()
    }
    fn duration(&self) -> Result<Duration, PlayerError> {
        self.0.duration()
    }
    fn seek(&self, position: Duration) -> Result<(), PlayerError> {
        self.0.seek(position)
    }
    fn set_volume(&self, gain: f32) -> Result<(), PlayerError> {
        self.0.set_volume(gain)
    }
    fn apply_tracker_settings(&self, settings: &TrackerSettings) -> Result<(), PlayerError> {
        self.0.apply_tracker_settings(settings)
    }
    fn on_end(&self, callback: Box<dyn Fn() + Send>) -> Result<Box<dyn Any + Send>, PlayerError> {
        self.0.on_end(callback)
    }
}

impl AudioBackend for MockBackend {
    fn open(&self, path: &Path) -> Result<Box<dyn BackendChannel>, PlayerError> {
        if self
            .fail_paths
            .lock()
            .unwrap()
            .contains(&path.to_path_buf())
        {
            return Err(PlayerError::NoCurrentTrack);
        }
        let channel = Arc::new(MockChannel::new(self.duration));
        *self.last_opened.lock().unwrap() = Some(Arc::clone(&channel));
        Ok(Box::new(SharedChannel(channel)))
    }

    fn set_tracker_resampling_quality(&self, _quality: u8) -> Result<(), PlayerError> {
        Ok(())
    }
}

/// Ticks `player` until `condition` is true or ~2 seconds pass (the open
/// happens on a real background thread even for the mock).
fn wait_until(player: &mut Player, mut condition: impl FnMut(&Player) -> bool) {
    for _ in 0..2000 {
        player.tick();
        if condition(player) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("condition not met in time");
}

fn drain_events(player: &Player) -> Vec<PlayerEvent> {
    player.events().try_iter().collect()
}

#[test]
fn replace_and_play_opens_off_thread_and_starts_playing() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.replace_and_play(vec![PathBuf::from("a.mp3"), PathBuf::from("b.mp3")], 0);

    wait_until(&mut player, |p| p.state() == PlaybackState::Playing);

    assert_eq!(player.current_path(), Some(Path::new("a.mp3")));
    let events = drain_events(&player);
    assert!(events.contains(&PlayerEvent::QueueChanged));
    assert!(matches!(
        events.iter().find(|e| matches!(e, PlayerEvent::TrackStarted { .. })),
        Some(PlayerEvent::TrackStarted { path, queue_index: 0 }) if path == Path::new("a.mp3")
    ));
}

#[test]
fn play_pause_toggles_state_without_reopening() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.replace_and_play(vec![PathBuf::from("a.mp3")], 0);
    wait_until(&mut player, |p| p.state() == PlaybackState::Playing);

    player.play_pause();
    assert_eq!(player.state(), PlaybackState::Paused);
    player.play_pause();
    assert_eq!(player.state(), PlaybackState::Playing);
}

#[test]
fn track_end_auto_advances_to_the_next_queue_entry() {
    let backend = MockBackend::new(Duration::from_millis(50));
    let mock = backend.clone();
    let mut player = Player::new(Arc::new(backend));
    player.replace_and_play(vec![PathBuf::from("a.mp3"), PathBuf::from("b.mp3")], 0);
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("a.mp3"))
    });

    mock.end_current_track();
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("b.mp3"))
    });

    let events = drain_events(&player);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, PlayerEvent::TrackEnded { path } if path == Path::new("a.mp3")))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        PlayerEvent::PlayFinished { path, .. } if path == Path::new("a.mp3")
    )));
}

#[test]
fn track_end_with_repeat_one_replays_the_same_track() {
    let backend = MockBackend::new(Duration::from_millis(50));
    let mock = backend.clone();
    let mut player = Player::new(Arc::new(backend));
    player.set_repeat_mode(emusic_player::RepeatMode::One);
    player.replace_and_play(vec![PathBuf::from("a.mp3"), PathBuf::from("b.mp3")], 0);
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("a.mp3"))
    });

    mock.end_current_track();
    // The track is dropped and reopened (off-thread) even for repeat-one,
    // so wait for it to actually come back rather than relying on `state`,
    // which never leaves `Playing` in this scenario.
    wait_until(&mut player, |p| p.current_path().is_none());
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("a.mp3"))
    });
}

#[test]
fn queue_runs_out_without_repeat_and_stops() {
    let backend = MockBackend::new(Duration::from_millis(30));
    let mock = backend.clone();
    let mut player = Player::new(Arc::new(backend));
    player.replace_and_play(vec![PathBuf::from("only.mp3")], 0);
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("only.mp3"))
    });

    mock.end_current_track();
    wait_until(&mut player, |p| p.state() == PlaybackState::Stopped);
}

#[test]
fn open_failure_emits_error_event_and_stops() {
    let backend = MockBackend::new(Duration::from_secs(5));
    backend.fail_for("broken.mp3");
    let mut player = Player::new(Arc::new(backend));
    player.replace_and_play(vec![PathBuf::from("broken.mp3")], 0);

    // `state()` starts at (and, on failure, returns to) `Stopped`, so it
    // can't be used as a wait condition here; instead give the worker
    // thread a generous window to report back and then check the events.
    for _ in 0..500 {
        player.tick();
        std::thread::sleep(Duration::from_millis(2));
    }
    let saw_error = drain_events(&player)
        .iter()
        .any(|e| matches!(e, PlayerEvent::Error(_)));
    assert!(
        saw_error,
        "opening a failing path should emit an Error event"
    );
    assert_eq!(player.current_path(), None);
}

#[test]
fn previous_restarts_current_track_after_the_threshold() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.replace_and_play(vec![PathBuf::from("a.mp3"), PathBuf::from("b.mp3")], 1);
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("b.mp3"))
    });

    // Simulate having played past the 3s restart threshold by seeking.
    player.seek(Duration::from_secs(5));
    player.previous();
    // Still on "b.mp3": previous() should have restarted it, not moved to
    // "a.mp3", because playback was past the threshold.
    assert_eq!(player.current_path(), Some(Path::new("b.mp3")));
}

#[test]
fn previous_moves_to_the_previous_track_within_the_threshold() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.replace_and_play(vec![PathBuf::from("a.mp3"), PathBuf::from("b.mp3")], 1);
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("b.mp3"))
    });

    player.previous();
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("a.mp3"))
    });
}

#[test]
fn play_next_inserts_right_after_the_current_track() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.replace_and_play(
        vec![
            PathBuf::from("a.mp3"),
            PathBuf::from("b.mp3"),
            PathBuf::from("c.mp3"),
        ],
        0,
    );
    wait_until(&mut player, |p| {
        p.current_path() == Some(Path::new("a.mp3"))
    });

    player.play_next(PathBuf::from("urgent.mp3"));

    let queue: Vec<_> = player.queue_paths().map(Path::to_path_buf).collect();
    assert_eq!(
        queue,
        vec![
            PathBuf::from("a.mp3"),
            PathBuf::from("urgent.mp3"),
            PathBuf::from("b.mp3"),
            PathBuf::from("c.mp3"),
        ]
    );
    // Still playing "a.mp3"; play_next doesn't disturb current playback.
    assert_eq!(player.current_path(), Some(Path::new("a.mp3")));
}

#[test]
fn play_next_with_nothing_playing_just_appends() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));

    player.play_next(PathBuf::from("only.mp3"));

    let queue: Vec<_> = player.queue_paths().map(Path::to_path_buf).collect();
    assert_eq!(queue, vec![PathBuf::from("only.mp3")]);
}

#[test]
fn set_volume_is_clamped_and_readable() {
    let backend = MockBackend::new(Duration::from_secs(5));
    let mut player = Player::new(Arc::new(backend));
    player.set_volume(2.0);
    assert_eq!(player.volume(), 1.0);
    player.set_volume(-1.0);
    assert_eq!(player.volume(), 0.0);
}

#[test]
fn waker_is_invoked_on_events() {
    let backend = MockBackend::new(Duration::from_secs(5));
    let mut player = Player::new(Arc::new(backend));
    let woken = Arc::new(AtomicBool::new(false));
    let woken_clone = Arc::clone(&woken);
    player.set_waker(Arc::new(move || woken_clone.store(true, Ordering::SeqCst)));

    player.replace_and_play(vec![PathBuf::from("a.mp3")], 0);
    assert!(
        woken.load(Ordering::SeqCst),
        "QueueChanged should have woken the UI"
    );
}

/// Starts the next track and waits until it (or a stop) shows up, returning
/// the new current path.
fn advance_and_wait(player: &mut Player) -> Option<PathBuf> {
    player.next();
    for _ in 0..2000 {
        player.tick();
        if player.state() == PlaybackState::Stopped {
            return None;
        }
        if let Some(path) = player.current_path() {
            return Some(path.to_path_buf());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("next track did not start in time");
}

#[test]
fn scoped_shuffle_plays_every_track_once_without_repeating() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.play_shuffled(
        vec![
            PathBuf::from("a.mp3"),
            PathBuf::from("b.mp3"),
            PathBuf::from("c.mp3"),
        ],
        "All tracks",
    );
    wait_until(&mut player, |p| p.state() == PlaybackState::Playing);

    let mut played = Vec::new();
    played.push(player.current_path().unwrap().to_path_buf());
    while let Some(path) = advance_and_wait(&mut player) {
        played.push(path);
    }

    assert_eq!(played.len(), 3, "each track should play exactly once");
    played.sort();
    played.dedup();
    assert_eq!(played.len(), 3, "no track may repeat within a cycle");
    assert_eq!(player.shuffle_scope(), Some("All tracks"));
}

#[test]
fn scoped_shuffle_previous_walks_back_through_history() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.play_shuffled(
        vec![PathBuf::from("a.mp3"), PathBuf::from("b.mp3")],
        "scope",
    );
    wait_until(&mut player, |p| p.state() == PlaybackState::Playing);
    let first = player.current_path().unwrap().to_path_buf();
    let second = advance_and_wait(&mut player).unwrap();

    player.previous();
    wait_until(&mut player, |p| p.current_path() == Some(first.as_path()));
    // The forward history is replayed by `next`.
    let forward = advance_and_wait(&mut player).unwrap();
    assert_eq!(forward, second);
}

#[test]
fn scoped_shuffle_repeat_all_starts_a_new_cycle() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.set_repeat_mode(emusic_player::RepeatMode::All);
    player.play_shuffled(
        vec![PathBuf::from("a.mp3"), PathBuf::from("b.mp3")],
        "scope",
    );
    wait_until(&mut player, |p| p.state() == PlaybackState::Playing);

    assert!(advance_and_wait(&mut player).is_some(), "second track");
    assert!(
        advance_and_wait(&mut player).is_some(),
        "repeat-all should reshuffle and keep playing"
    );
}

#[test]
fn turning_shuffle_off_ends_the_scope() {
    let backend = MockBackend::new(Duration::from_secs(10));
    let mut player = Player::new(Arc::new(backend));
    player.play_shuffled(
        vec![PathBuf::from("a.mp3"), PathBuf::from("b.mp3")],
        "scope",
    );
    wait_until(&mut player, |p| p.state() == PlaybackState::Playing);

    player.set_shuffle(false);
    assert_eq!(player.shuffle_scope(), None);
    assert!(!player.shuffle());
    assert!(
        player.current_path().is_some(),
        "the current track keeps playing"
    );
}

#[test]
fn unreadable_tracks_are_skipped_not_stopped_in_a_scope() {
    let backend = MockBackend::new(Duration::from_millis(30));
    backend.fail_for("bad.mp3");
    let mock = backend.clone();
    let mut player = Player::new(Arc::new(backend));
    player.play_shuffled(
        vec![PathBuf::from("good.mp3"), PathBuf::from("bad.mp3")],
        "scope",
    );
    wait_until(&mut player, |p| p.state() == PlaybackState::Playing);

    let mut events = Vec::new();
    for _ in 0..4000 {
        player.tick();
        events.extend(drain_events(&player));
        if player.state() == PlaybackState::Stopped {
            break;
        }
        mock.end_current_track();
        std::thread::sleep(Duration::from_millis(1));
    }

    assert!(
        events.iter().any(
            |e| matches!(e, PlayerEvent::TrackSkipped { path } if path == Path::new("bad.mp3"))
        ),
        "the unreadable track should be reported as skipped"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, PlayerEvent::TrackStarted { path, .. } if path == Path::new("good.mp3"))
        ),
        "the readable track should still play"
    );
}
