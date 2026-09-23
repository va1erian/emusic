//! MIDI playback support: recognising MIDI files and choosing the soundfont
//! BASSMIDI renders them with.
//!
//! BASSMIDI is a software synthesizer, not a wrapper around the operating
//! system's MIDI synth: without a soundfont it opens MIDI files (so their
//! length is known) but renders silence. The soundfont is the one the user
//! configured, else the first one found next to the BASS DLLs.

use std::path::{Path, PathBuf};

/// File extensions played through BASSMIDI.
const MIDI_EXTENSIONS: &[&str] = &["mid", "midi"];

/// Soundfont extensions BASSMIDI can load.
const SOUNDFONT_EXTENSIONS: &[&str] = &["sf2", "sf3", "sfz"];

fn has_extension(path: &Path, allowed: &[&str]) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| allowed.iter().any(|a| a.eq_ignore_ascii_case(ext)))
}

/// Whether `path`'s extension names a MIDI file.
pub fn is_midi_file(path: &Path) -> bool {
    has_extension(path, MIDI_EXTENSIONS)
}

/// Why a configured soundfont can't be used, or `None` when it looks fine.
///
/// Only checks that the file exists and has a soundfont extension; BASSMIDI
/// validates the contents itself when it loads the font.
pub fn soundfont_problem(path: &Path) -> Option<&'static str> {
    if !path.is_file() {
        Some("File not found.")
    } else if !has_extension(path, SOUNDFONT_EXTENSIONS) {
        Some("Not a soundfont (expected .sf2, .sf3 or .sfz).")
    } else {
        None
    }
}

/// Picks the soundfont to play MIDI with: `configured` when it's usable,
/// otherwise the first (alphabetically) soundfont file directly inside one
/// of `search_dirs`.
pub fn resolve_soundfont(configured: Option<&Path>, search_dirs: &[PathBuf]) -> Option<PathBuf> {
    if let Some(path) = configured
        && soundfont_problem(path).is_none()
    {
        return Some(path.to_path_buf());
    }
    search_dirs.iter().find_map(|dir| first_soundfont_in(dir))
}

fn first_soundfont_in(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && has_extension(path, SOUNDFONT_EXTENSIONS))
        .collect();
    found.sort();
    found.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emusic-midi-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn midi_extensions_are_recognised_case_insensitively() {
        for ext in ["mid", "MID", "Midi", "midi"] {
            assert!(is_midi_file(Path::new(&format!("song.{ext}"))));
        }
        assert!(!is_midi_file(Path::new("song.mp3")));
        assert!(!is_midi_file(Path::new("song")));
    }

    #[test]
    fn soundfont_problem_reports_missing_and_wrong_files() {
        let dir = scratch_dir("problem");
        let font = dir.join("gm.sf2");
        let text = dir.join("notes.txt");
        std::fs::write(&font, b"x").expect("write");
        std::fs::write(&text, b"x").expect("write");

        assert_eq!(soundfont_problem(&font), None);
        assert!(soundfont_problem(&dir.join("absent.sf2")).is_some());
        assert!(soundfont_problem(&text).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn configured_soundfont_wins_over_discovered_ones() {
        let dir = scratch_dir("configured");
        let configured = dir.join("mine.sf2");
        std::fs::write(&configured, b"x").expect("write");
        std::fs::write(dir.join("a.sf2"), b"x").expect("write");

        let resolved = resolve_soundfont(Some(&configured), std::slice::from_ref(&dir));
        assert_eq!(resolved, Some(configured));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_configured_soundfont_falls_back_to_discovery() {
        let dir = scratch_dir("fallback");
        std::fs::write(dir.join("b.sf3"), b"x").expect("write");
        std::fs::write(dir.join("a.SF2"), b"x").expect("write");

        let resolved = resolve_soundfont(Some(&dir.join("gone.sf2")), std::slice::from_ref(&dir));
        assert_eq!(resolved, Some(dir.join("a.SF2")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_resolves_without_a_soundfont() {
        let dir = scratch_dir("none");
        std::fs::write(dir.join("readme.txt"), b"x").expect("write");

        assert_eq!(resolve_soundfont(None, std::slice::from_ref(&dir)), None);
        assert_eq!(resolve_soundfont(None, &[]), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
