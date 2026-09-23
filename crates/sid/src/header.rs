#![forbid(unsafe_code)]

//! PSID/RSID header parsing, in plain Rust (no engine needed).
//!
//! The header layout is the de-facto standard described in the HVSC
//! `SID_file_format.txt`: a big-endian, fixed-offset header followed by the
//! tune's C64 program. Text fields are 32-byte, NUL-padded Windows-1252
//! strings. Only the fields needed to describe and select a tune are parsed
//! here; the engine reads the rest itself.

use crate::error::SidError;

/// Size of a PSID v1 header (`0x76`).
pub const HEADER_SIZE_V1: usize = 0x76;
/// Size of a PSID/RSID v2+ header (`0x7C`).
pub const HEADER_SIZE_V2: usize = 0x7C;

/// Which variant the file declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidFormat {
    /// A PlaySID/PSID tune, run in the "simulated player call" environment.
    Psid,
    /// A RealSID tune, run in a full C64/CIA/VIC environment.
    Rsid,
}

/// The SID chip revision a tune asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipModel {
    /// MOS 6581 (the original, "warmer", with the famous filter).
    Mos6581,
    /// MOS 8580 (the later, cleaner revision).
    Mos8580,
}

/// The video standard / clock a tune asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// PAL (50 Hz, ~0.985 MHz CPU).
    Pal,
    /// NTSC (60 Hz, ~1.023 MHz CPU).
    Ntsc,
}

/// The parsed, engine-independent description of a SID tune.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidHeader {
    /// PSID or RSID.
    pub format: SidFormat,
    /// Header version (`1` for PSID v1, `2..=4` otherwise).
    pub version: u8,
    /// Declared header size in bytes.
    pub header_size: usize,
    /// C64 load address; `0` means "take it from the first two data bytes".
    pub load_address: u16,
    /// Address of the init routine.
    pub init_address: u16,
    /// Address of the play routine; `0` for tunes driven by the init routine.
    pub play_address: u16,
    /// Number of subtunes (`1..=256`).
    pub subtunes: u16,
    /// Default subtune (`1..=subtunes`).
    pub default_subtune: u16,
    /// Per-subtune speed flags (bit set = CIA timing, clear = VIC).
    pub speed: [u8; 4],
    /// Tune title (Windows-1252).
    pub title: String,
    /// Tune author (Windows-1252).
    pub author: String,
    /// Release information (Windows-1252).
    pub released: String,
    /// Chip model hinted by the header, or `None` when unspecified.
    pub chip_model: Option<ChipModel>,
    /// Clock hinted by the header, or `None` when unspecified.
    pub clock: Option<Clock>,
}

impl SidHeader {
    /// Parses the header at the start of `data`.
    ///
    /// Only the header is read; the trailing program data is not validated
    /// beyond the header's own bounds.
    pub fn parse(data: &[u8]) -> Result<Self, SidError> {
        if data.len() < HEADER_SIZE_V1 {
            return Err(SidError::TooSmall);
        }

        let format = if &data[0..4] == b"PSID" {
            SidFormat::Psid
        } else if &data[0..4] == b"RSID" {
            SidFormat::Rsid
        } else {
            return Err(SidError::BadMagic);
        };

        let version = data[0x05];
        let header_size = usize::from(be16(data, 0x06));
        if header_size < HEADER_SIZE_V1 || header_size > data.len() {
            return Err(SidError::BadHeaderSize(header_size));
        }

        let subtunes = be16(data, 0x0E).max(1);
        let default_subtune = be16(data, 0x10).clamp(1, subtunes);

        let mut speed = [0u8; 4];
        speed.copy_from_slice(&data[0x12..0x16]);

        let (chip_model, clock) = if version >= 2 && header_size >= HEADER_SIZE_V2 {
            parse_model_and_clock(data[0x77])
        } else {
            (None, None)
        };

        Ok(Self {
            format,
            version,
            header_size,
            load_address: be16(data, 0x08),
            init_address: be16(data, 0x0A),
            play_address: be16(data, 0x0C),
            subtunes,
            default_subtune,
            speed,
            title: decode_win1252(&data[0x16..0x36]),
            author: decode_win1252(&data[0x36..0x56]),
            released: decode_win1252(&data[0x56..0x76]),
            chip_model,
            clock,
        })
    }
}

/// Reads a big-endian `u16` at `offset`.
fn be16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

/// Decodes the model/clock bit fields of `ModelFormatStandard` (`$77`).
///
/// Bits 5-4 select the SID1 model (`01` = 6581, `10` = 8580, `11` = both);
/// bits 3-2 select the video standard (`01` = PAL, `10` = NTSC, `11` = both).
/// "Unknown" and "both" both map to `None`, i.e. "let the engine decide".
fn parse_model_and_clock(flags: u8) -> (Option<ChipModel>, Option<Clock>) {
    let chip_model = match (flags & 0x30) >> 4 {
        0b01 => Some(ChipModel::Mos6581),
        0b10 => Some(ChipModel::Mos8580),
        _ => None,
    };
    let clock = match (flags & 0x0C) >> 2 {
        0b01 => Some(Clock::Pal),
        0b10 => Some(Clock::Ntsc),
        _ => None,
    };
    (chip_model, clock)
}

/// Decodes a NUL-padded Windows-1252 field into a `String`, stopping at the
/// first NUL (trailing bytes are ignored).
fn decode_win1252(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|&b| win1252_char(b)).collect()
}

/// Maps one Windows-1252 byte to its Unicode scalar.
///
/// Bytes `0x00..=0x7F` and `0xA0..=0xFF` match Unicode directly (as in
/// Latin-1); only `0x80..=0x9F` differ. The five undefined slots in that range
/// (per the WHATWG encoding spec) fall back to the same code point.
fn win1252_char(byte: u8) -> char {
    match byte {
        0x80 => '\u{20AC}',
        0x82 => '\u{201A}',
        0x83 => '\u{0192}',
        0x84 => '\u{201E}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02C6}',
        0x89 => '\u{2030}',
        0x8A => '\u{0160}',
        0x8B => '\u{2039}',
        0x8C => '\u{0152}',
        0x8E => '\u{017D}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201C}',
        0x94 => '\u{201D}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02DC}',
        0x99 => '\u{2122}',
        0x9A => '\u{0161}',
        0x9B => '\u{203A}',
        0x9C => '\u{0153}',
        0x9E => '\u{017E}',
        0x9F => '\u{0178}',
        _ => char::from(byte),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal, valid v2 header with the given song counts and flags.
    fn header_bytes(version: u8, subtunes: u16, default: u16, model_flags: u8) -> Vec<u8> {
        let mut data = vec![0u8; HEADER_SIZE_V2];
        data[0..4].copy_from_slice(b"PSID");
        data[0x05] = version;
        data[0x06..0x08].copy_from_slice(&(HEADER_SIZE_V2 as u16).to_be_bytes());
        data[0x08..0x0A].copy_from_slice(&0x1000u16.to_be_bytes());
        data[0x0A..0x0C].copy_from_slice(&0x1000u16.to_be_bytes());
        data[0x0C..0x0E].copy_from_slice(&0x1000u16.to_be_bytes());
        data[0x0E..0x10].copy_from_slice(&subtunes.to_be_bytes());
        data[0x10..0x12].copy_from_slice(&default.to_be_bytes());
        data[0x77] = model_flags;
        data
    }

    #[test]
    fn parses_a_psid_v2_header() {
        let mut data = header_bytes(2, 1, 1, 0);
        data[0x16..0x1B].copy_from_slice(b"Title");
        data[0x36..0x3C].copy_from_slice(b"Author");
        data[0x56..0x5F].copy_from_slice(b"Released!");

        let header = SidHeader::parse(&data).expect("valid header");
        assert_eq!(header.format, SidFormat::Psid);
        assert_eq!(header.version, 2);
        assert_eq!(header.header_size, HEADER_SIZE_V2);
        assert_eq!(header.load_address, 0x1000);
        assert_eq!(header.init_address, 0x1000);
        assert_eq!(header.play_address, 0x1000);
        assert_eq!(header.title, "Title");
        assert_eq!(header.author, "Author");
        assert_eq!(header.released, "Released!");
    }

    #[test]
    fn enumerates_subtunes_and_clamps_the_default() {
        let header = SidHeader::parse(&header_bytes(2, 3, 2, 0)).expect("valid header");
        assert_eq!(header.subtunes, 3);
        assert_eq!(header.default_subtune, 2);
    }

    #[test]
    fn a_zero_default_subtune_falls_back_to_one() {
        let header = SidHeader::parse(&header_bytes(2, 5, 0, 0)).expect("valid header");
        assert_eq!(header.default_subtune, 1);
    }

    #[test]
    fn an_out_of_range_default_subtune_is_clamped() {
        let header = SidHeader::parse(&header_bytes(2, 3, 99, 0)).expect("valid header");
        assert_eq!(header.default_subtune, 3);
    }

    #[test]
    fn parses_model_and_clock_flags() {
        // SID1 = 6581 (01), PAL (01).
        let header = SidHeader::parse(&header_bytes(2, 1, 1, 0x14)).expect("valid header");
        assert_eq!(header.chip_model, Some(ChipModel::Mos6581));
        assert_eq!(header.clock, Some(Clock::Pal));

        // SID1 = 8580 (10), NTSC (10).
        let header = SidHeader::parse(&header_bytes(2, 1, 1, 0x28)).expect("valid header");
        assert_eq!(header.chip_model, Some(ChipModel::Mos8580));
        assert_eq!(header.clock, Some(Clock::Ntsc));
    }

    #[test]
    fn unknown_model_and_clock_bits_map_to_none() {
        let header = SidHeader::parse(&header_bytes(2, 1, 1, 0)).expect("valid header");
        assert_eq!(header.chip_model, None);
        assert_eq!(header.clock, None);
    }

    #[test]
    fn a_v1_header_has_no_model_or_clock_fields() {
        let mut data = header_bytes(1, 1, 1, 0x14);
        data[0x06..0x08].copy_from_slice(&(HEADER_SIZE_V1 as u16).to_be_bytes());
        data.truncate(HEADER_SIZE_V1);
        let header = SidHeader::parse(&data).expect("valid header");
        assert_eq!(header.chip_model, None);
        assert_eq!(header.clock, None);
    }

    #[test]
    fn recognizes_rsid() {
        let mut data = header_bytes(2, 1, 1, 0);
        data[0..4].copy_from_slice(b"RSID");
        let header = SidHeader::parse(&data).expect("valid header");
        assert_eq!(header.format, SidFormat::Rsid);
    }

    #[test]
    fn rejects_bad_magic_and_short_files() {
        let mut data = header_bytes(2, 1, 1, 0);
        data[0] = b'X';
        assert_eq!(SidHeader::parse(&data), Err(SidError::BadMagic));
        assert_eq!(SidHeader::parse(&[0u8; 4]), Err(SidError::TooSmall));
    }

    #[test]
    fn decodes_windows_1252_text() {
        // "R\x92verie" uses a curly apostrophe (0x92 -> U+2019).
        let mut data = header_bytes(2, 1, 1, 0);
        data[0x16..0x1D].copy_from_slice(b"R\x92verie");
        let header = SidHeader::parse(&data).expect("valid header");
        assert_eq!(header.title, "R\u{2019}verie");
    }
}
