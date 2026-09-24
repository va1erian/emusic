//! Deterministic helper functions for generating seeded mock library data.
//!
//! Kept separate from [`super::data`] so the data module stays focused on
//! the high-level generation pipeline and stays under the project's file-size
//! guidelines.

use rand::Rng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;

/// Deterministic fake technical metadata for streamed formats. Tracker
/// modules leave these fields empty because BASS reports them differently.
pub fn stream_metadata(
    format: &str,
    tracker_formats: &[&str],
    rng: &mut ChaCha8Rng,
) -> (String, Option<u32>, Option<u32>, Option<u8>, Option<u8>) {
    if tracker_formats.contains(&format) {
        return (format.to_uppercase(), None, None, None, None);
    }

    let codec = match format {
        "mp3" => "MPEG-1 Layer III",
        "flac" => "FLAC",
        "ogg" => "Ogg Vorbis",
        "m4a" => "AAC",
        "wav" => "PCM",
        _ => "Unknown",
    }
    .to_string();

    let bitrate = match format {
        "mp3" => Some(rng.gen_range(192..=320)),
        "flac" => Some(rng.gen_range(700..=1200)),
        "ogg" => Some(rng.gen_range(128..=256)),
        "m4a" => Some(rng.gen_range(128..=256)),
        "wav" => Some(rng.gen_range(800..=1500)),
        _ => None,
    };

    let sample_rate = Some(match format {
        "wav" if rng.gen_bool(0.5) => 48_000,
        "wav" => 44_100,
        _ => 44_100,
    });

    let bit_depth = match format {
        "flac" | "wav" => Some(if rng.gen_bool(0.8) { 16 } else { 24 }),
        _ => None,
    };

    let channels = Some(if rng.gen_bool(0.9) { 2 } else { 1 });

    (codec, bitrate, sample_rate, bit_depth, channels)
}

pub fn weighted_play_count(rng: &mut ChaCha8Rng) -> u32 {
    // Most tracks rarely played, a handful very frequently - gives
    // `most_played` a non-trivial ranking.
    if rng.gen_bool(0.05) {
        rng.gen_range(50..300)
    } else if rng.gen_bool(0.2) {
        rng.gen_range(5..50)
    } else {
        rng.gen_range(0..5)
    }
}

pub fn person_name(
    name_words: &[&str],
    name_nouns: &[&str],
    rng: &mut ChaCha8Rng,
    i: u32,
) -> String {
    format!(
        "{} {}",
        name_words[(i as usize).wrapping_mul(7) % name_words.len()],
        name_nouns[rng.gen_range(0..name_nouns.len())]
    )
}

pub fn phrase(name_words: &[&str], name_nouns: &[&str], rng: &mut ChaCha8Rng, salt: u32) -> String {
    let a = name_words[(salt as usize).wrapping_mul(3) % name_words.len()];
    let b = name_nouns[rng.gen_range(0..name_nouns.len())];
    format!("{a} {b}")
}

pub fn long_title(rng: &mut ChaCha8Rng, name_words: &[&str]) -> String {
    let words: Vec<&str> = (0..9).map(|_| *name_words.choose(rng).unwrap()).collect();
    format!("{} (Extended Unabridged Remaster)", words.join(" "))
}

pub fn tracker_title(rng: &mut ChaCha8Rng, name_words: &[&str], name_nouns: &[&str]) -> String {
    let instrument = ["lead synth", "amiga bass", "fm organ", "chip lead", "noise"]
        .choose(rng)
        .unwrap();
    let salt: u32 = rng.r#gen();
    format!(
        "{} - module msg: \"{}\"",
        phrase(name_words, name_nouns, rng, salt),
        instrument
    )
}

pub fn build_folders(rng: &mut ChaCha8Rng, artists: &[String]) -> Vec<String> {
    let roots = ["D:/Music/Library", "D:/Music/Imports", "D:/Music/Tracker"];
    let mut folders = Vec::new();
    for artist in artists {
        let root = roots.choose(rng).unwrap();
        let safe_artist = artist.replace(' ', "_");
        if rng.gen_bool(0.5) {
            folders.push(format!("{root}/{safe_artist}"));
        } else {
            folders.push(format!("{root}/{safe_artist}/Disc1"));
        }
    }
    folders
}

/// Candidate "minutes ago" values used both for the play history list and
/// for tracks' `last_played` field.
const MINUTES_AGO_CHOICES: &[u32] = &[1, 5, 20, 45, 90, 180, 600, 1440, 2880, 10080];

pub fn minutes_ago(rng: &mut ChaCha8Rng) -> &'static u32 {
    MINUTES_AGO_CHOICES.choose(rng).unwrap()
}
