//! Minimal, safe parser for PSID/RSID headers.
//!
//! Only the fields needed for indexing are read: format version, subtune
//! count, default subtune and the textual name/author/released fields. The
//! client performs the actual emulation.

/// Parsed PSID/RSID header fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidHeader {
    /// `PSID` or `RSID`.
    pub magic: String,
    /// Header version.
    pub version: u16,
    /// Number of subtunes (always at least 1).
    pub songs: u32,
    /// Default starting subtune.
    pub start_song: u32,
    /// Tune name, when present.
    pub name: Option<String>,
    /// Author, when present.
    pub author: Option<String>,
    /// Released/copyright field, when present.
    pub released: Option<String>,
}

const MIN_HEADER: usize = 0x76;

/// Parses a SID header, or returns `None` when the data is not a SID file.
pub fn parse(data: &[u8]) -> Option<SidHeader> {
    if data.len() < 4 {
        return None;
    }
    let magic = std::str::from_utf8(&data[0..4]).ok()?;
    if magic != "PSID" && magic != "RSID" {
        return None;
    }
    if data.len() < MIN_HEADER {
        return None;
    }
    let version = be_u16(data, 4)?;
    let songs = be_u16(data, 14)? as u32;
    let start_song = be_u16(data, 16)? as u32;
    Some(SidHeader {
        magic: magic.to_string(),
        version,
        songs: songs.max(1),
        start_song: start_song.max(1),
        name: text_field(data, 22, 32),
        author: text_field(data, 54, 32),
        released: text_field(data, 86, 32),
    })
}

fn be_u16(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// Reads a zero-terminated ASCII field and trims trailing padding.
fn text_field(data: &[u8], offset: usize, len: usize) -> Option<String> {
    let slice = data.get(offset..offset + len)?;
    let end = slice
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(slice.len());
    let text = std::str::from_utf8(&slice[..end]).ok()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(magic: &[u8], songs: u16, start: u16) -> Vec<u8> {
        let mut data = vec![0u8; MIN_HEADER];
        data[0..4].copy_from_slice(magic);
        data[4..6].copy_from_slice(&2u16.to_be_bytes());
        data[14..16].copy_from_slice(&songs.to_be_bytes());
        data[16..18].copy_from_slice(&start.to_be_bytes());
        data[22..26].copy_from_slice(b"Test");
        data[54..59].copy_from_slice(b"Anon.");
        data
    }

    #[test]
    fn parses_a_psid_header() {
        let header = parse(&header(b"PSID", 5, 2)).expect("parsed");
        assert_eq!(header.magic, "PSID");
        assert_eq!(header.songs, 5);
        assert_eq!(header.start_song, 2);
        assert_eq!(header.name.as_deref(), Some("Test"));
        assert_eq!(header.author.as_deref(), Some("Anon."));
        assert_eq!(header.released, None);
    }

    #[test]
    fn parses_rsid() {
        assert_eq!(parse(&header(b"RSID", 1, 1)).unwrap().magic, "RSID");
    }

    #[test]
    fn rejects_non_sid_data() {
        assert!(parse(b"ID3\x04not a sid").is_none());
        assert!(parse(b"PSID").is_none());
        assert!(parse(&[]).is_none());
    }

    #[test]
    fn zero_song_count_is_clamped_to_one() {
        let header = parse(&header(b"PSID", 0, 0)).unwrap();
        assert_eq!(header.songs, 1);
        assert_eq!(header.start_song, 1);
    }
}
