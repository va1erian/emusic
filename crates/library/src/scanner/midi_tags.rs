//! MIDI file metadata via BASS.
//!
//! MIDI files carry no tags lofty can read, so they are opened decode-only
//! through the `bassmidi` plugin with `PRESCAN` to get an accurate length.
//! Rendering the length needs no soundfont, so scanning works even before
//! one is configured.

use bass::{Bass, BassError, Channel, StreamFlags};

use emusic_core::Track;

use super::walk::FoundFile;

/// Flags for metadata-only MIDI opening (see the module docs).
fn metadata_flags() -> StreamFlags {
    StreamFlags::DECODE | StreamFlags::PRESCAN
}

/// Reads the duration of one MIDI file. The title is the file name without
/// its extension and the genre is `MIDI`.
pub(crate) fn read_midi(bass: &Bass, file: &FoundFile, now: i64) -> Result<Track, BassError> {
    let stream = bass.open_stream(&file.path, metadata_flags())?;
    let seconds = stream.length_seconds()?;

    let mut track = super::untagged_track(file, now);
    track.duration_ms = (seconds * 1000.0).round() as u32;
    track.title = file
        .path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty());
    track.genre = Some("MIDI".to_string());
    Ok(track)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_flags_are_decode_only_but_prescanned() {
        let flags = metadata_flags();
        assert!(flags.contains(StreamFlags::DECODE));
        assert!(flags.contains(StreamFlags::PRESCAN));
    }
}
