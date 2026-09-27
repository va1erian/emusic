//! PSID/RSID header parsing.
//!
//! Adapted from the `sidera` crate's `sidplay` example (MIT, © Sébastien
//! Béchet).

/// The parsed parts of a PSID/RSID header we need (multi-byte fields are
/// big-endian on disk).
pub(crate) struct Psid {
    pub(crate) load_addr: u16,
    pub(crate) init_addr: u16,
    pub(crate) play_addr: u16,
    pub(crate) start_song: u16,
    /// One bit per sub-song: 0 = vertical-blank timing, 1 = CIA #1 Timer A.
    pub(crate) speed_bits: u32,
    /// `Some(true)` = NTSC, `Some(false)` = PAL, `None` = unspecified.
    pub(crate) prefer_ntsc: Option<bool>,
    /// `Some(true)` = 8580, `Some(false)` = 6581, `None` = unspecified.
    pub(crate) prefer_8580: Option<bool>,
    /// The C64 bytes loaded into RAM at `load_addr`.
    pub(crate) data: Vec<u8>,
}

impl Psid {
    /// Parses a PSID/RSID image.
    pub(crate) fn parse(raw: &[u8]) -> Result<Psid, String> {
        if raw.len() < 0x80 {
            return Err("file too short to be a PSID".into());
        }
        let magic = &raw[0..4];
        if magic != b"PSID" && magic != b"RSID" {
            return Err(format!("not a PSID/RSID file (magic {magic:?})"));
        }
        let be16 = |o: usize| u16::from_be_bytes([raw[o], raw[o + 1]]);
        let version = be16(0x04);
        let data_offset = be16(0x06) as usize;
        let mut load_addr = be16(0x08);
        let init_addr = be16(0x0a);
        let play_addr = be16(0x0c);
        let start_song = be16(0x10);
        let speed_bits = u32::from_be_bytes([raw[0x12], raw[0x13], raw[0x14], raw[0x15]]);

        // PSID v2NG+ carries clock/model hints in the flags word at $76; a v1
        // header already has tune data there, so only v2+ flags are trusted.
        let (prefer_ntsc, prefer_8580) = if version >= 2 {
            let flags = be16(0x76);
            let ntsc = match (flags >> 2) & 0b11 {
                0b01 => Some(false),
                0b10 => Some(true),
                _ => None,
            };
            let is8580 = match (flags >> 4) & 0b11 {
                0b01 => Some(false),
                0b10 => Some(true),
                _ => None,
            };
            (ntsc, is8580)
        } else {
            (None, None)
        };

        if data_offset >= raw.len() {
            return Err("dataOffset past end of file".into());
        }
        let mut data = &raw[data_offset..];
        // loadAddress == 0 means the real load address is the first two data
        // bytes (little-endian) and the payload follows.
        if load_addr == 0 {
            if data.len() < 2 {
                return Err("missing inline load address".into());
            }
            load_addr = u16::from_le_bytes([data[0], data[1]]);
            data = &data[2..];
        }
        Ok(Psid {
            load_addr,
            init_addr,
            play_addr,
            start_song,
            speed_bits,
            prefer_ntsc,
            prefer_8580,
            data: data.to_vec(),
        })
    }

    /// Whether this (0-based) sub-song is CIA-timed rather than vertical-blank.
    pub(crate) fn is_cia_timed(&self, song0: u8) -> bool {
        let bit = song0.min(31);
        self.speed_bits & (1u32 << bit) != 0
    }
}
