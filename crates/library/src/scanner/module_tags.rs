//! Tracker module metadata via BASS.
//!
//! Modules (MOD/XM/IT/S3M/...) have no tags lofty can read; instead they are
//! opened decode-only with `BASS_MusicLoad` (`NOSAMPLE | PRESCAN`) and their
//! name, message, instrument and sample names are read from
//! `BASS_ChannelGetTags`. `NOSAMPLE` skips loading sample data — only the
//! metadata and an accurate pre-scanned length are needed.

use bass::{Bass, BassError, Channel, MusicFlags, MusicTags};

use emusic_core::Track;

use super::paths;
use super::walk::FoundFile;

/// Flags for metadata-only module loading (see the module docs).
fn metadata_flags() -> MusicFlags {
    MusicFlags::DECODE | MusicFlags::NOSAMPLE | MusicFlags::PRESCAN
}

/// Reads name, length and message/instrument/sample tags for one module.
///
/// The returned track has `kind` set to module, a `Tracker (<FMT>)` genre
/// derived from the file extension, the module's message plus instrument
/// and sample names as its comment, and no bitrate/sample rate (those only
/// exist for the mixing output, not the file itself).
pub(crate) fn read_module(bass: &Bass, file: &FoundFile, now: i64) -> Result<Track, BassError> {
    let music = bass.open_music(&file.path, metadata_flags(), 0)?;
    let seconds = music.length_seconds()?;
    let tags = music.tags();

    let mut track = super::untagged_track(file, now);
    track.duration_ms = (seconds * 1000.0).round() as u32;
    track.title = tags
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(String::from);
    track.genre = Some(format!("Tracker ({})", extension_name(&file.path)));
    track.comment = module_comment(&tags);
    Ok(track)
}

/// The uppercased extension, e.g. `XM` for `tune.xm`.
fn extension_name(path: &std::path::Path) -> String {
    paths::extension(path).unwrap_or_default().to_uppercase()
}

/// The module message plus instrument and sample name lists as a comment.
///
/// BASS reports an entry (possibly an empty string) for every instrument
/// and sample slot; empty names are dropped.
fn module_comment(tags: &MusicTags) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(message) = tags
        .message
        .as_deref()
        .filter(|message| !message.is_empty())
    {
        parts.push(message.to_string());
    }
    let instruments = named_strings(&tags.instruments);
    if !instruments.is_empty() {
        parts.push(format!("Instruments: {}", instruments.join("; ")));
    }
    let samples = named_strings(&tags.samples);
    if !samples.is_empty() {
        parts.push(format!("Samples: {}", samples.join("; ")));
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

/// Returns the non-empty entries from `names` as owned strings.
fn named_strings(names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(String::as_str)
        .filter(|name| !name.is_empty())
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_flags_are_decode_only_without_samples_but_prescanned() {
        let flags = metadata_flags();
        assert!(flags.contains(MusicFlags::DECODE));
        assert!(flags.contains(MusicFlags::NOSAMPLE));
        assert!(flags.contains(MusicFlags::PRESCAN));
        assert!(!flags.contains(MusicFlags::FLOAT));
    }

    #[test]
    fn module_comment_combines_message_instruments_and_samples() {
        let tags = MusicTags {
            name: Some("tune".to_string()),
            message: Some("hello tracker".to_string()),
            instruments: vec!["lead".to_string(), "bass".to_string()],
            samples: vec!["kick".to_string()],
        };
        assert_eq!(
            module_comment(&tags).as_deref(),
            Some("hello tracker\nInstruments: lead; bass\nSamples: kick")
        );
    }

    #[test]
    fn module_comment_is_none_for_an_untagged_module() {
        assert_eq!(module_comment(&MusicTags::default()), None);
    }
}
