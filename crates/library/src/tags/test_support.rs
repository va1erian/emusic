//! Shared fixtures for the tag tests: a synthesized tagged WAV and the temp
//! directories each test edits in.

use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::tag::{ItemKey, Tag, TagExt, TagType};

/// A small but valid 44100 Hz mono 16-bit PCM WAV, generated in-memory.
pub(crate) fn minimal_wav() -> Vec<u8> {
    const RATE: u32 = 44_100;
    const SAMPLES: usize = 1024;
    let data_len = (SAMPLES * 2) as u32;
    let byte_rate = RATE * 2;

    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVE");
    out.extend(b"fmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(RATE.to_le_bytes());
    out.extend(byte_rate.to_le_bytes());
    out.extend(2u16.to_le_bytes()); // block align
    out.extend(16u16.to_le_bytes()); // bits per sample
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    out.extend(std::iter::repeat_n(0u8, data_len as usize));
    out
}

/// The unique temp directory a test named `name` may use.
pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emusic-tags-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes the tagged WAV fixture and returns the copy each test edits.
pub(crate) fn tagged_fixture(name: &str) -> PathBuf {
    let dir = temp_dir(name);
    let fixture = dir.join("fixture.wav");
    std::fs::write(&fixture, minimal_wav()).unwrap();

    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::TrackTitle, "Seed Title".to_string());
    tag.insert_text(ItemKey::TrackArtist, "Seed Artist".to_string());
    tag.insert_text(ItemKey::Comment, "Seed Comment".to_string());
    tag.save_to_path(&fixture, WriteOptions::default()).unwrap();

    let target = dir.join("track.wav");
    std::fs::copy(&fixture, &target).unwrap();
    target
}

/// Removes the temp directory a fixture/test file lives in.
pub(crate) fn cleanup(path: &Path) {
    if let Some(dir) = path.parent() {
        std::fs::remove_dir_all(dir).ok();
    }
}
