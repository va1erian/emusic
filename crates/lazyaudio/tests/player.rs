//! `emusic_player::Player` on `LazyBackend`, the way the app drives it: the
//! same checks as the BASS integration test, plus the queue moving on, pause
//! and seek.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::fixture;
use emusic_lazyaudio::LazyBackend;
use emusic_lazyaudio::output::MemorySink;
use emusic_player::{PlaybackState, Player, PlayerEvent};

fn player(speed: f64) -> (Player, MemorySink) {
    let sink = MemorySink::new(8192, 2048, speed);
    let player = Player::new(Arc::new(LazyBackend::new(Arc::new(sink.clone()))));
    (player, sink)
}

/// Ticks `player` until `done` holds, failing after `limit`.
fn tick_until(player: &mut Player, limit: Duration, what: &str, done: impl Fn(&Player) -> bool) {
    let start = Instant::now();
    loop {
        player.tick();
        if done(player) {
            return;
        }
        assert!(start.elapsed() < limit, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn plays_a_track_to_its_end() {
    let (mut player, _sink) = player(10.0);
    let path = fixture("cbr.mp3");
    player.replace_and_play(vec![path.clone()], 0);
    tick_until(&mut player, Duration::from_secs(5), "Playing", |p| {
        p.state() == PlaybackState::Playing
    });
    assert_eq!(player.current_path(), Some(path.as_path()));
    let duration = player.duration().expect("duration");
    assert!((duration.as_secs_f64() - 1.0).abs() < 0.03, "{duration:?}");
    assert!(player.seek_supported());
    tick_until(&mut player, Duration::from_secs(5), "Stopped", |p| {
        p.state() == PlaybackState::Stopped
    });
    let finished = player
        .events()
        .try_iter()
        .any(|event| matches!(event, PlayerEvent::PlayFinished { .. }));
    assert!(finished, "expected a PlayFinished accounting event");
}

#[test]
fn the_queue_moves_on_to_the_next_track() {
    let (mut player, _sink) = player(10.0);
    player.replace_and_play(vec![fixture("cbr.mp3"), fixture("mono.mp3")], 0);
    let second = fixture("mono.mp3");
    tick_until(
        &mut player,
        Duration::from_secs(10),
        "the second track",
        |p| p.current_path() == Some(second.as_path()) && p.state() == PlaybackState::Playing,
    );
    tick_until(&mut player, Duration::from_secs(5), "the end", |p| {
        p.state() == PlaybackState::Stopped
    });
}

#[test]
fn a_file_that_does_not_play_reports_an_error() {
    let (mut player, _sink) = player(10.0);
    let unplayable = std::env::temp_dir().join("emusic_lazyaudio_player_unplayable.ogg");
    std::fs::write(&unplayable, b"OggS not really").unwrap();
    player.replace_and_play(vec![unplayable.clone()], 0);
    // The file opens on the player's worker: wait for its verdict.
    let start = Instant::now();
    let error = loop {
        player.tick();
        if let Some(error) = player.events().try_iter().find_map(|event| match event {
            PlayerEvent::Error(error) => Some(error),
            _ => None,
        }) {
            break error;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "no Error event");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(error.to_string().contains("not a playable MP3"), "{error}");
    assert_eq!(player.state(), PlaybackState::Stopped);
    let _ = std::fs::remove_file(unplayable);
}

#[test]
fn pause_holds_the_position_and_seek_moves_it() {
    let (mut player, sink) = player(1.0);
    player.replace_and_play(vec![fixture("two_tones.mp3")], 0);
    tick_until(&mut player, Duration::from_secs(5), "some playback", |p| {
        p.position()
            .is_some_and(|at| at > Duration::from_millis(200))
    });
    player.play_pause();
    player.tick();
    assert_eq!(player.state(), PlaybackState::Paused);
    let paused_at = player.position().unwrap();
    std::thread::sleep(Duration::from_millis(150));
    player.tick();
    assert_eq!(player.position().unwrap(), paused_at);

    player.seek(Duration::from_millis(1700));
    player.tick();
    let at = player.position().unwrap();
    assert!((at.as_secs_f64() - 1.7).abs() < 0.01, "{at:?}");
    let heard = sink.recording().samples.len();
    player.play_pause();
    tick_until(&mut player, Duration::from_secs(5), "the end", |p| {
        p.state() == PlaybackState::Stopped
    });
    // About 0.3 s of the second tone after the seek.
    let after = sink.recording().samples.len() - heard;
    assert!((2 * 11_025..2 * 22_050).contains(&after), "{after} samples");
}
