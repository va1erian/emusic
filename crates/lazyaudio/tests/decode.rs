//! The MP3 decoder against the generated fixtures: formats, tags, damage,
//! seeking and arbitrary bytes.

mod common;

use std::io::Cursor;
use std::time::Duration;

use common::{XorShift, assert_tone, bytes, decode_all, first_channel, open};
use emusic_lazyaudio::decode::{Error, Mp3Decoder, SeekPrecision};

/// Opens `name`, checks its format and length, and that it is a 440 Hz tone.
fn check_tone_fixture(name: &str, rate: u32, channels: u16) {
    let mut decoder = open(bytes(name));
    let info = *decoder.info();
    assert_eq!((info.rate, info.channels), (rate, channels), "{name}");
    let duration = info.duration.expect("the header gives the length");
    assert!(
        (duration.as_secs_f64() - 1.0).abs() < 0.03,
        "{name}: {duration:?}"
    );
    assert_eq!(info.seek, SeekPrecision::Exact);
    let samples = decode_all(&mut decoder);
    let frames = samples.len() / usize::from(channels);
    assert_eq!(Some(frames as u64), info.frames, "{name}: decoded frames");
    let mono = first_channel(&samples, usize::from(channels));
    assert_tone(&mono[frames / 10..frames * 9 / 10], rate, 440.0);
}

#[test]
fn decodes_cbr() {
    check_tone_fixture("cbr.mp3", 44_100, 2);
}

#[test]
fn decodes_vbr_with_a_xing_header() {
    check_tone_fixture("vbr.mp3", 44_100, 2);
}

#[test]
fn decodes_past_id3v2_and_id3v1_tags() {
    check_tone_fixture("tags.mp3", 44_100, 2);
}

#[test]
fn decodes_mono() {
    check_tone_fixture("mono.mp3", 44_100, 1);
}

#[test]
fn decodes_mpeg2_at_22050_hz() {
    check_tone_fixture("mpeg2.mp3", 22_050, 2);
}

#[test]
fn a_truncated_stream_ends_early_without_an_error() {
    let mut data = bytes("cbr.mp3");
    data.truncate(data.len() * 6 / 10);
    let mut decoder = open(data);
    let samples = decode_all(&mut decoder);
    let frames = samples.len() / 2;
    assert!(frames > 44_100 / 3 && frames < 44_100 * 8 / 10, "{frames}");
    assert_tone(
        &first_channel(&samples, 2)[1000..frames - 1000],
        44_100,
        440.0,
    );
}

#[test]
fn skips_garbage_in_front_of_the_stream() {
    let mut rng = XorShift(0x5eed);
    let mut data: Vec<u8> = (0..3000).map(|_| (rng.next() & 0xff) as u8).collect();
    // No accidental frame sync in the junk: keep 0xFF bytes out of it.
    data.iter_mut().filter(|b| **b == 0xff).for_each(|b| *b = 0);
    data.extend(bytes("cbr.mp3"));
    let mut decoder = open(data);
    let samples = decode_all(&mut decoder);
    let frames = samples.len() / 2;
    assert!(frames > 40_000, "{frames}");
    assert_tone(
        &first_channel(&samples, 2)[4000..frames - 4000],
        44_100,
        440.0,
    );
}

#[test]
fn a_seek_lands_on_the_exact_sample() {
    let mut reference = open(bytes("two_tones.mp3"));
    let all = decode_all(&mut reference);
    for target in [0.0, 0.25, 1.0, 1.5, 1.999] {
        let mut decoder = open(bytes("two_tones.mp3"));
        let landed = decoder.seek(Duration::from_secs_f64(target)).expect("seek");
        let frame = (target * 44_100.0) as usize;
        assert_eq!(
            (landed.as_secs_f64() * 44_100.0).round(),
            frame as f64,
            "{target}"
        );
        let mut block = vec![0.0; 512];
        let count = decoder.read(&mut block).expect("read");
        let expected = &all[frame * 2..(frame * 2 + count).min(all.len())];
        assert_eq!(&block[..expected.len()], expected, "seek to {target}s");
    }
}

#[test]
fn a_seek_reaches_the_second_tone() {
    let mut decoder = open(bytes("two_tones.mp3"));
    decoder.seek(Duration::from_millis(1200)).expect("seek");
    let samples = decode_all(&mut decoder);
    let mono = first_channel(&samples, 2);
    assert_tone(&mono[..mono.len() * 3 / 4], 44_100, 660.0);
    // And back to the first.
    decoder.seek(Duration::from_millis(100)).expect("seek");
    let mut block = vec![0.0; 2 * 22_050];
    let count = decoder.read(&mut block).expect("read");
    assert_tone(&first_channel(&block[..count], 2), 44_100, 440.0);
}

#[test]
fn a_seek_past_the_end_leaves_nothing() {
    let mut decoder = open(bytes("cbr.mp3"));
    decoder.seek(Duration::from_secs(30)).expect("seek");
    assert!(decode_all(&mut decoder).is_empty());
}

#[test]
fn rejects_what_is_not_mp3() {
    let wav = b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x44\xac\x00\x00\x88\x58\x01\x00\x02\x00\x10\x00data\x00\x00\x00\x00";
    for data in [Vec::new(), b"hello, world".to_vec(), wav.to_vec()] {
        match Mp3Decoder::open(Cursor::new(data)) {
            Err(Error::Unsupported(_)) => {}
            Err(other) => panic!("expected Unsupported, got {other}"),
            Ok(_) => panic!("opened something that is not an MP3"),
        }
    }
}

/// Decodes arbitrary and mutated bytes: nothing may panic or loop forever.
#[test]
fn survives_arbitrary_bytes() {
    let cases: usize = std::env::var("FUZZ_CASES")
        .ok()
        .and_then(|cases| cases.parse().ok())
        .unwrap_or(200);
    let seed = std::env::var("FUZZ_SEED")
        .ok()
        .and_then(|seed| u64::from_str_radix(seed.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x00e6_0517);
    let mut rng = XorShift(seed);
    let base = bytes("cbr.mp3");
    for case in 0..cases {
        let data = if case % 2 == 0 {
            let len = rng.below(4096);
            (0..len).map(|_| (rng.next() & 0xff) as u8).collect()
        } else {
            let mut data = base.clone();
            for _ in 0..1 + rng.below(64) {
                let at = rng.below(data.len());
                data[at] = (rng.next() & 0xff) as u8;
            }
            data.truncate(rng.below(data.len()) + 1);
            data
        };
        let Ok(mut decoder) = Mp3Decoder::open(Cursor::new(data)) else {
            continue;
        };
        if rng.below(2) == 0 {
            let _ = decoder.seek(Duration::from_millis(rng.below(2000) as u64));
        }
        let mut block = vec![0.0; 4096];
        // A stream of at most a few seconds: bound the reads.
        for _ in 0..200 {
            match decoder.read(&mut block) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }
}
