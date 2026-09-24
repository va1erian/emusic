//! Shared fixtures and pump helpers for the library backend tests.
//!
//! Background work (scans, tag edits) completes on its own thread, so the
//! tests below drive [`LibraryBackend::tick`] in a loop until the expected
//! state appears, with a generous timeout for slow (network) machines.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::LibraryBackend;
use crate::library_api::{LibraryDataSource, TrackInfo};

/// A unique temp directory for a test named `tag`, removed first if it
/// already exists.
pub(crate) fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("emusic-lib-{tag}-{}-{nanos}", std::process::id()))
}

/// Writes a minimal valid 16-bit PCM mono WAV with a short silent tone.
pub(crate) fn write_wav(path: &Path, sample_rate: u32, seconds: u32) {
    let samples = sample_rate * seconds;
    let data_len = samples * 2; // 16-bit mono
    let byte_rate = sample_rate * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..samples {
        let phase = (i as f32 / sample_rate as f32) * 440.0 * std::f32::consts::TAU;
        let sample = (phase.sin() * 1000.0) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

/// Pumps the backend until the background scan produces a track, up to a
/// generous timeout (network drives are slow; local temp dirs are not).
pub(crate) fn wait_for_tracks(backend: &mut LibraryBackend) -> Vec<TrackInfo> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        backend.tick();
        if !backend.tracks().is_empty() {
            return backend.tracks().to_vec();
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("scan did not produce a track within the timeout");
}

/// Pumps the backend until it reports exactly `expected` tracks.
pub(crate) fn wait_for_track_count(backend: &mut LibraryBackend, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        backend.tick();
        if backend.tracks().len() == expected {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!(
        "expected {expected} tracks, still have {} after the timeout",
        backend.tracks().len()
    );
}

/// The current time as a Unix timestamp (seconds), or 0 when unavailable.
pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
