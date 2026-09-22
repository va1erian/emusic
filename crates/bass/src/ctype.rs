//! Readable names for BASS `BASS_CHANNELINFO.ctype` values.
//!
//! Every `BASS_CTYPE_*` constant from the official `bass.h` is mapped,
//! along with the well-known add-on codes (`bassflac.h`, `bassopus.h`,
//! `basswma.h` and the documented codes used by the other add-ons:
//! ALAC, WavPack, APE, MPC, WebM, ...). Anything unrecognized falls back
//! to `Unknown (0x....)` with the raw value, so new codes are never
//! silently misrepresented.

use crate::ffi::types::Dword;

const CTYPE_STREAM: Dword = 0x0001_0000;
const CTYPE_MUSIC: Dword = 0x0002_0000;
const CTYPE_WAV: Dword = 0x0004_0000;
const MO3_FLAG: Dword = 0x0000_0100;
const TOP_CATEGORY_MASK: Dword = 0xFFFF_0000;

/// Maps a `BASS_CHANNELINFO.ctype` value to a readable description like
/// `"WAV PCM"`, `"MP3"`, `"FLAC"`, `"Opus"`, `"IT"` or `"Music (MO3)"`.
///
/// Streaming category codes (`CTYPE_STREAM_*` and add-ons) are matched
/// exactly using the official header values, except for WAV-formatted
/// streams whose low word is the codec (`BASS_CTYPE_STREAM_WAV |
/// codec`) and MO3 modules (`BASS_CTYPE_MUSIC_* | MO3_FLAG`).
pub fn format_name(ctype: Dword) -> String {
    if let Some(name) = exact_name(ctype) {
        return name.to_string();
    }
    if let Some(name) = (ctype & MO3_FLAG != 0)
        .then_some(ctype & !MO3_FLAG)
        .and_then(mo3_base)
    {
        return format!("{name} (MO3)");
    }
    if (ctype & CTYPE_WAV) != 0 {
        return wav_name(ctype);
    }
    match ctype & TOP_CATEGORY_MASK {
        CTYPE_STREAM => format!("Stream (0x{ctype:08x})"),
        CTYPE_MUSIC => format!("Music (0x{ctype:08x})"),
        _ => format!("Unknown (0x{ctype:08x})"),
    }
}

fn exact_name(ctype: Dword) -> Option<&'static str> {
    match ctype {
        0x1 => Some("Sample"),
        0x2 => Some("Record"),
        0x0001_0000 => Some("Stream"),
        0x0001_0002 => Some("OGG"),
        0x0001_0003 => Some("MP1"),
        0x0001_0004 => Some("MP2"),
        0x0001_0005 => Some("MP3"),
        0x0001_0006 => Some("AIFF"),
        0x0001_0007 => Some("Core Audio"),
        0x0001_0008 => Some("Media Foundation"),
        0x0001_0009 => Some("Audio Manager"),
        0x0001_000a => Some("Sample Stream"),
        0x0001_0300 => Some("WMA"),
        0x0001_0301 => Some("WMA (MP3 payload)"),
        0x0001_0500 => Some("OptimFROG"),
        0x0001_0600 => Some("AC3"),
        0x0001_0700 => Some("APE"),
        0x0001_0900 => Some("FLAC"),
        0x0001_0901 => Some("FLAC (OGG)"),
        0x0001_0a00 => Some("AAC"),
        0x0001_0b00 => Some("MPC"),
        0x0001_0c00 => Some("WebM"),
        0x0001_0d00 => Some("Speex"),
        0x0001_0e00 => Some("ALAC"),
        0x0001_0f00 => Some("WavPack"),
        0x0001_1200 => Some("Opus"),
        0x0001_8000 => Some("Dummy"),
        0x0001_8001 => Some("Device"),
        0x0005_0001 => Some("WAV PCM"),
        0x0005_0003 => Some("WAV Float"),
        0x0002_0000 => Some("MOD"),
        0x0002_0001 => Some("MTM"),
        0x0002_0002 => Some("S3M"),
        0x0002_0003 => Some("XM"),
        0x0002_0004 => Some("IT"),
        _ => None,
    }
}

/// How a `ctype` names the WAV codec when it has the `CTYPE_WAV` flag.
fn wav_name(ctype: Dword) -> String {
    match ctype & 0x0000_FFFF {
        0x1 => "WAV PCM".to_string(),
        0x3 => "WAV Float".to_string(),
        _ => format!("WAV (codec 0x{:04x})", ctype & 0x0000_FFFF),
    }
}

/// The `BASS_CTYPE_MUSIC_*` module format of a MO3-flagged `ctype`.
fn mo3_base(ctype: Dword) -> Option<&'static str> {
    match ctype {
        0x0002_0000 => Some("MOD"),
        0x0002_0001 => Some("MTM"),
        0x0002_0002 => Some("S3M"),
        0x0002_0003 => Some("XM"),
        0x0002_0004 => Some("IT"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_variants() {
        assert_eq!(format_name(0x0005_0001), "WAV PCM");
        assert_eq!(format_name(0x0005_0003), "WAV Float");
        assert_eq!(format_name(0x0004_0001), "WAV PCM");
        assert_eq!(format_name(0x0004_0003), "WAV Float");
        assert_eq!(format_name(0x0004_0011), "WAV (codec 0x0011)");
    }

    #[test]
    fn codec_names() {
        assert_eq!(format_name(0x0001_0005), "MP3");
        assert_eq!(format_name(0x0001_0002), "OGG");
        assert_eq!(format_name(0x0001_0900), "FLAC");
        assert_eq!(format_name(0x0001_1200), "Opus");
        assert_eq!(format_name(0x0001_0300), "WMA");
        assert_eq!(format_name(0x0001_0e00), "ALAC");
        assert_eq!(format_name(0x0001_0f00), "WavPack");
        assert_eq!(format_name(0x0001_0700), "APE");
        assert_eq!(format_name(0x0001_0b00), "MPC");
        assert_eq!(format_name(0x0001_0c00), "WebM");
    }

    #[test]
    fn music_names() {
        assert_eq!(format_name(0x0002_0004), "IT");
        assert_eq!(format_name(0x0002_0002), "S3M");
        // MO3 is a flag ORed into the module type.
        assert_eq!(format_name(0x0002_0104), "IT (MO3)");
        assert_eq!(format_name(0x0002_0100), "MOD (MO3)");
    }

    #[test]
    fn unknown_values_keep_the_raw_hex() {
        assert_eq!(format_name(0x9999_9999), "Unknown (0x99999999)");
        assert_eq!(format_name(0x0001_7777), "Stream (0x00017777)");
        assert_eq!(format_name(0x0002_0999), "Music (0x00020999)");
    }
}
