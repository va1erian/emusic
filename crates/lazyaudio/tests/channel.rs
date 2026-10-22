//! `LazyBackend` channels against a `MemorySink`: what reaches the sound
//! system, the clock, pause, seek, volume and the end of the track.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::{assert_tone, bytes, decode_all, first_channel, fixture, open};
use emusic_lazyaudio::LazyBackend;
use emusic_lazyaudio::output::MemorySink;
use emusic_player::backend::{AudioBackend, BackendChannel, SeekSupport};

/// Queue and period sizes like `audiod`'s, played 10 times faster than real
/// time.
fn sink() -> MemorySink {
    MemorySink::new(8192, 2048, 10.0)
}

fn channel(sink: &MemorySink, name: &str) -> Box<dyn BackendChannel> {
    LazyBackend::new(Arc::new(sink.clone()))
        .open(&fixture(name))
        .expect("open the fixture")
}

/// Waits until `done` holds, failing after `limit`.
fn wait_for(limit: Duration, what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < limit, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The recording as `f32`, without the silence padding the last period.
fn recorded(sink: &MemorySink, frames: usize, channels: usize) -> Vec<f32> {
    let recording = sink.recording();
    recording.samples[..frames * channels]
        .iter()
        .map(|&s| f32::from(s) / f32::from(i16::MAX))
        .collect()
}

/// What the decoder makes of `name`, as the sink receives it.
fn reference(name: &str) -> Vec<i16> {
    decode_all(&mut open(bytes(name)))
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
        .collect()
}

#[test]
fn plays_the_whole_track_and_reports_its_end() {
    let sink = sink();
    let channel = channel(&sink, "cbr.mp3");
    let ended = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&ended);
    let _guard = channel
        .on_end(Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("register on_end");
    assert!(channel.is_active());
    channel.play(false).expect("play");
    wait_for(Duration::from_secs(5), "the end", || {
        ended.load(Ordering::SeqCst) > 0
    });
    assert!(!channel.is_active());
    assert_eq!(ended.load(Ordering::SeqCst), 1);
    let expected = reference("cbr.mp3");
    let recording = sink.recording();
    assert_eq!(&recording.samples[..expected.len()], &expected[..]);
    // Only silence follows (what played of the padding of the last period).
    assert!(recording.samples[expected.len()..].iter().all(|&s| s == 0));
    assert_eq!(recording.opened, vec![(44_100, 2)]);
    assert_eq!(channel.position().unwrap(), channel.duration().unwrap());
}

#[test]
fn the_clock_follows_what_played() {
    let sink = MemorySink::new(8192, 2048, 1.0);
    let channel = channel(&sink, "two_tones.mp3");
    channel.play(false).expect("play");
    std::thread::sleep(Duration::from_millis(600));
    let position = channel.position().unwrap().as_secs_f64();
    let played = sink.recording().samples.len() as f64 / 2.0 / 44_100.0;
    // The clock is what reached the sink, not what was decoded ahead.
    assert!((position - played).abs() < 0.06, "{position} vs {played}");
    assert!((0.3..0.7).contains(&position), "{position}");
}

#[test]
fn a_pause_loses_and_repeats_nothing() {
    let sink = sink();
    let channel = channel(&sink, "cbr.mp3");
    channel.play(false).expect("play");
    wait_for(Duration::from_secs(5), "some playback", || {
        channel.position().unwrap() > Duration::from_millis(300)
    });
    channel.pause().expect("pause");
    let paused_at = channel.position().unwrap();
    let heard = sink.recording().samples.len();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(channel.position().unwrap(), paused_at);
    assert_eq!(sink.recording().samples.len(), heard);
    assert!(channel.is_active(), "a paused channel is still active");

    channel.play(false).expect("resume");
    wait_for(Duration::from_secs(5), "the end", || !channel.is_active());
    let expected = reference("cbr.mp3");
    let recording = sink.recording();
    // The first stream played whole periods only; nothing between them is
    // missing or doubled.
    let joined: Vec<i16> = recording
        .samples
        .iter()
        .copied()
        .take(expected.len())
        .collect();
    assert_eq!(joined, expected);
    assert_eq!(recording.opened.len(), 2);
}

#[test]
fn a_seek_moves_playback_and_the_clock() {
    let sink = sink();
    let channel = channel(&sink, "two_tones.mp3");
    assert_eq!(channel.capabilities().seek, SeekSupport::Exact);
    channel.seek(Duration::from_millis(1250)).expect("seek");
    assert_eq!(channel.position().unwrap(), Duration::from_millis(1250));
    channel.play(false).expect("play");
    wait_for(Duration::from_secs(5), "the end", || !channel.is_active());
    let frames = 44_100 * 3 / 4;
    let samples = recorded(&sink, frames, 2);
    assert_tone(
        &first_channel(&samples, 2)[..frames * 9 / 10],
        44_100,
        660.0,
    );
}

#[test]
fn a_seek_while_playing_drops_what_was_queued() {
    let sink = sink();
    let channel = channel(&sink, "two_tones.mp3");
    channel.play(false).expect("play");
    wait_for(Duration::from_secs(5), "some playback", || {
        channel.position().unwrap() > Duration::from_millis(200)
    });
    channel.seek(Duration::from_millis(1500)).expect("seek");
    let before = sink.recording().samples.len();
    wait_for(Duration::from_secs(5), "the end", || !channel.is_active());
    let recording = sink.recording();
    let after: Vec<f32> = recording.samples[before..before + 2 * 11_025]
        .iter()
        .map(|&s| f32::from(s))
        .collect();
    assert_tone(&first_channel(&after, 2), 44_100, 660.0);
}

#[test]
fn volume_reaches_the_stream() {
    let sink = sink();
    let channel = channel(&sink, "cbr.mp3");
    channel.set_volume(0.5).expect("volume before playing");
    channel.play(false).expect("play");
    wait_for(Duration::from_secs(5), "the end", || !channel.is_active());
    let recording = sink.recording();
    assert_eq!(recording.gains.first(), Some(&(0, 0.5)));
}

#[test]
fn stop_ends_the_channel_without_reporting_an_end() {
    let sink = sink();
    let channel = channel(&sink, "cbr.mp3");
    let ended = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&ended);
    let _guard = channel
        .on_end(Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }))
        .unwrap();
    channel.play(false).expect("play");
    channel.stop().expect("stop");
    assert!(!channel.is_active());
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(ended.load(Ordering::SeqCst), 0);
}

#[test]
fn the_visualizer_sees_what_played() {
    let sink = MemorySink::new(8192, 2048, 1.0);
    let channel = channel(&sink, "cbr.mp3");
    assert!(channel.samples().is_none());
    channel.play(false).expect("play");
    wait_for(Duration::from_secs(5), "some playback", || {
        channel.position().unwrap() > Duration::from_millis(300)
    });
    let samples = channel.samples().expect("samples");
    assert_eq!(samples.len(), 1024);
    // About ten cycles: count them rather than measure the frequency.
    let cycles = common::frequency(&samples, 44_100) * 1024.0 / 44_100.0;
    assert!((9.0..=11.0).contains(&cycles.round()), "{cycles} cycles");
    let bins = channel.fft().expect("spectrum");
    assert_eq!(bins.len(), 512);
    // 440 Hz in 1024 points at 44.1 kHz is bin 10.
    let peak = (0..bins.len())
        .max_by(|&a, &b| bins[a].total_cmp(&bins[b]))
        .unwrap();
    assert!((9..=11).contains(&peak), "peak in bin {peak}");
    channel.stop().unwrap();
}

#[test]
fn files_that_are_not_mp3_do_not_open() {
    let sink = sink();
    let backend = LazyBackend::new(Arc::new(sink));
    let path = std::env::temp_dir().join("emusic_lazyaudio_not_mp3.flac");
    std::fs::write(&path, b"fLaC\0\0\0\x22 not really").unwrap();
    assert!(backend.open(&path).is_err());
    assert!(backend.open(&fixture("missing.mp3")).is_err());
    let _ = std::fs::remove_file(path);
}
