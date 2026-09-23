//! Integration test: the vendored cRSID engine renders audible PCM from a
//! synthetic PSID tune. No third-party SID file is committed — the tune is a
//! hand-assembled 6502 stub — so there are no sample-licensing concerns.

use emusic_sid::header::HEADER_SIZE_V2;
use emusic_sid::{SidConfig, SidPlayer};

/// A minimal PSID v2 tune whose init routine programmes SID voice 1 for a
/// sustained triangle tone and whose play routine is a bare `RTS`.
///
/// The header claims `subtunes` songs with `default` selected; the tiny program
/// itself ignores the subtune, which is enough to exercise enumeration and
/// selection.
fn minimal_tone_psid(subtunes: u16, default: u16) -> Vec<u8> {
    // 6502, loaded at $1000:
    //   LDA #$0F / STA $D418   ; master volume
    //   LDA #$00 / STA $D405   ; attack/decay = 0
    //   LDA #$F0 / STA $D406   ; sustain = 15, release = 0
    //   LDA #$00 / STA $D400   ; frequency lo
    //   LDA #$10 / STA $D401   ; frequency hi
    //   LDA #$11 / STA $D404   ; triangle waveform + gate
    //   RTS
    let program: [u8; 31] = [
        0xA9, 0x0F, 0x8D, 0x18, 0xD4, 0xA9, 0x00, 0x8D, 0x05, 0xD4, 0xA9, 0xF0, 0x8D, 0x06, 0xD4,
        0xA9, 0x00, 0x8D, 0x00, 0xD4, 0xA9, 0x10, 0x8D, 0x01, 0xD4, 0xA9, 0x11, 0x8D, 0x04, 0xD4,
        0x60,
    ];
    let load = 0x1000u16;
    let play = load + program.len() as u16 - 1; // the trailing RTS

    let mut data = vec![0u8; HEADER_SIZE_V2];
    data[0..4].copy_from_slice(b"PSID");
    data[0x05] = 2; // PSID v2
    data[0x06..0x08].copy_from_slice(&(HEADER_SIZE_V2 as u16).to_be_bytes());
    data[0x08..0x0A].copy_from_slice(&load.to_be_bytes());
    data[0x0A..0x0C].copy_from_slice(&load.to_be_bytes());
    data[0x0C..0x0E].copy_from_slice(&play.to_be_bytes());
    data[0x0E..0x10].copy_from_slice(&subtunes.to_be_bytes());
    data[0x10..0x12].copy_from_slice(&default.to_be_bytes());
    data[0x16..0x1A].copy_from_slice(b"Tone");
    data[0x36..0x3A].copy_from_slice(b"Test");
    data[0x77] = 0x14; // SID1 = 6581, PAL
    data.extend_from_slice(&program);
    data
}

#[test]
fn renders_audible_pcm() {
    let mut player = SidPlayer::from_bytes(minimal_tone_psid(1, 1), 44_100, SidConfig::default())
        .expect("load synthetic tune");

    // Half a second is plenty for the envelope to open.
    let mut pcm = vec![0i16; 22_050];
    let written = player.render(&mut pcm);
    assert_eq!(written, pcm.len());

    let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    assert!(peak > 100, "expected audible output, peak was {peak}");
}

#[test]
fn reports_subtunes_from_the_header() {
    let player = SidPlayer::from_bytes(minimal_tone_psid(3, 2), 44_100, SidConfig::default())
        .expect("load synthetic tune");
    assert_eq!(player.subtune_count(), 3);
    assert_eq!(player.default_subtune(), 2);
    assert_eq!(player.current_subtune(), 2);
}

#[test]
fn clamps_subtune_selection() {
    let mut player = SidPlayer::from_bytes(minimal_tone_psid(2, 1), 44_100, SidConfig::default())
        .expect("load synthetic tune");
    player.select_subtune(99).expect("select subtune");
    assert_eq!(player.current_subtune(), 2);
}
