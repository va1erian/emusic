//! Shared helpers for the decoder and channel tests.

#![allow(dead_code)]

use std::io::Cursor;
use std::path::PathBuf;

use emusic_lazyaudio::decode::Mp3Decoder;

/// The path of fixture `name` (see `tests/fixtures/make.py`).
pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The bytes of fixture `name`.
pub fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).expect("read fixture")
}

/// Opens `bytes` from memory.
pub fn open(bytes: Vec<u8>) -> Mp3Decoder {
    Mp3Decoder::open(Cursor::new(bytes)).expect("open MP3")
}

/// Every sample `decoder` has left, interleaved.
pub fn decode_all(decoder: &mut Mp3Decoder) -> Vec<f32> {
    let mut all = Vec::new();
    let mut block = vec![0.0; 4096];
    loop {
        let count = decoder.read(&mut block).expect("decode");
        if count == 0 {
            return all;
        }
        all.extend_from_slice(&block[..count]);
    }
}

/// The first channel of interleaved `samples`.
pub fn first_channel(samples: &[f32], channels: usize) -> Vec<f32> {
    samples.iter().step_by(channels).copied().collect()
}

/// The frequency of a sine from its rising zero crossings.
pub fn frequency(mono: &[f32], rate: u32) -> f64 {
    let crossings = mono
        .windows(2)
        .filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0)
        .count();
    crossings as f64 * f64::from(rate) / mono.len() as f64
}

/// Asserts that `mono` is a sine of about `expected` Hz.
pub fn assert_tone(mono: &[f32], rate: u32, expected: f64) {
    let found = frequency(mono, rate);
    assert!(
        (found - expected).abs() < expected * 0.03,
        "expected ~{expected} Hz, found {found} Hz"
    );
}

/// A small deterministic generator for the seeded fuzz runs.
pub struct XorShift(pub u64);

impl XorShift {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}
