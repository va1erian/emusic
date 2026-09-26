//! Minimal title/channel extraction for common tracker module formats.
//!
//! The client renders modules with BASS; the server only needs enough
//! metadata to index and display them. Parsing is best-effort and never fails
//! a scan: unknown or truncated files simply yield no fields.

/// Fields extracted from a module header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleInfo {
    /// Song title, when the header stores one.
    pub title: Option<String>,
    /// Channel count, when the header stores it.
    pub channels: Option<u32>,
}

/// Parses a module header, dispatching on the format signature.
pub fn parse(data: &[u8]) -> ModuleInfo {
    if data.len() >= 17 && &data[0..17] == b"Extended Module: " {
        return parse_xm(data);
    }
    if data.len() >= 4 && &data[0..4] == b"IMPM" {
        return parse_it(data);
    }
    if data.len() >= 48 && &data[0x2c..0x30] == b"SCRM" {
        return parse_s3m(data);
    }
    parse_mod(data)
}

/// Classic ProTracker layout: title at offset 0, channel count from the
/// signature at offset 1080.
fn parse_mod(data: &[u8]) -> ModuleInfo {
    let title = text(data, 0, 20);
    let channels = if data.len() >= 1084 {
        match &data[1080..1084] {
            b"M.K." | b"M!K!" | b"FLT4" | b"FLT8" => Some(4),
            b"6CHN" => Some(6),
            b"8CHN" | b"OCTA" | b"OKTA" => Some(8),
            b"10CH" => Some(10),
            b"12CH" => Some(12),
            b"16CN" => Some(16),
            b"32CN" => Some(32),
            _ => None,
        }
    } else {
        None
    };
    ModuleInfo { title, channels }
}

/// XM: signature 0..17, title 17..37, channel count at offset 68.
fn parse_xm(data: &[u8]) -> ModuleInfo {
    ModuleInfo {
        title: text(data, 17, 20),
        channels: be_u16(data, 68).map(u32::from),
    }
}

/// IT: signature `IMPM`, title 4..30. Channel count is not in the header.
fn parse_it(data: &[u8]) -> ModuleInfo {
    ModuleInfo {
        title: text(data, 4, 26),
        channels: None,
    }
}

/// S3M: `SCRM` at 0x2C, title at 0.
fn parse_s3m(data: &[u8]) -> ModuleInfo {
    ModuleInfo {
        title: text(data, 0, 28),
        channels: None,
    }
}

fn be_u16(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn text(data: &[u8], offset: usize, len: usize) -> Option<String> {
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

    #[test]
    fn parses_mod_title_and_channels() {
        let mut data = vec![0u8; 1084];
        data[0..8].copy_from_slice(b"My Song\0");
        data[1080..1084].copy_from_slice(b"M.K.");
        let info = parse(&data);
        assert_eq!(info.title.as_deref(), Some("My Song"));
        assert_eq!(info.channels, Some(4));
    }

    #[test]
    fn parses_xm_title_and_channels() {
        let mut data = vec![0u8; 80];
        data[0..17].copy_from_slice(b"Extended Module: ");
        data[17..23].copy_from_slice(b"xmname");
        data[68..70].copy_from_slice(&8u16.to_le_bytes());
        let info = parse(&data);
        assert_eq!(info.title.as_deref(), Some("xmname"));
        assert_eq!(info.channels, Some(8));
    }

    #[test]
    fn parses_it_title() {
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(b"IMPM");
        data[4..12].copy_from_slice(b"it tune\0");
        assert_eq!(parse(&data).title.as_deref(), Some("it tune"));
    }

    #[test]
    fn truncated_data_yields_nothing() {
        assert_eq!(parse(b"\x00\x01").title, None);
        assert_eq!(parse(b"").channels, None);
    }
}
