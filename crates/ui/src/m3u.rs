//! Extended M3U (`#EXTM3U`) playlist export.

use std::path::{Component, Path, PathBuf};

use crate::library_api::TrackInfo;

/// Renders `tracks` as an extended M3U document for a file that will live
/// in `base_dir`.
///
/// Paths beneath `base_dir` are written relative to it (so the playlist
/// and its music can move together); all others stay absolute. Lines end
/// in `\n`. The result is meant to be written as UTF-8 (`.m3u8`).
pub fn export(tracks: &[&TrackInfo], base_dir: &Path) -> String {
    let mut out = String::from("#EXTM3U\n");
    for track in tracks {
        out.push_str(&format!(
            "#EXTINF:{},{}\n",
            track.duration.as_secs(),
            display_name(track)
        ));
        out.push_str(&entry_path(&track.path, base_dir));
        out.push('\n');
    }
    out
}

/// `Artist - Title`, or whichever of the two is present. The text goes on
/// one line, so line breaks are flattened to spaces.
fn display_name(track: &TrackInfo) -> String {
    let name = match (track.artist.is_empty(), track.title.is_empty()) {
        (false, false) => format!("{} - {}", track.artist, track.title),
        (true, false) => track.title.clone(),
        (false, true) => track.artist.clone(),
        (true, true) => Path::new(&track.path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    name.replace(['\r', '\n'], " ")
}

fn entry_path(path: &str, base_dir: &Path) -> String {
    match relative_to(Path::new(path), base_dir) {
        Some(rel) => rel.to_string_lossy().into_owned(),
        None => path.to_owned(),
    }
}

/// `path` relative to `base` when it lies beneath it, comparing
/// components case-insensitively (Windows paths).
fn relative_to(path: &Path, base: &Path) -> Option<PathBuf> {
    let mut path_parts = path.components();
    for base_part in base.components() {
        let part = path_parts.next()?;
        if !same_component(part, base_part) {
            return None;
        }
    }
    let rest: PathBuf = path_parts.collect();
    let is_plain = rest.components().all(|c| matches!(c, Component::Normal(_)));
    (is_plain && rest.components().next().is_some()).then_some(rest)
}

fn same_component(a: Component, b: Component) -> bool {
    a.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn track(path: &str, artist: &str, title: &str, secs: u64) -> TrackInfo {
        TrackInfo {
            path: path.into(),
            artist: artist.into(),
            title: title.into(),
            duration: Duration::from_secs(secs),
            ..TrackInfo::default()
        }
    }

    #[test]
    fn writes_header_and_extinf_lines() {
        let t = track("/music/a.flac", "Artist", "Song", 185);
        let text = export(&[&t], Path::new("/elsewhere"));
        assert_eq!(text, "#EXTM3U\n#EXTINF:185,Artist - Song\n/music/a.flac\n");
    }

    #[test]
    fn empty_playlist_is_just_the_header() {
        assert_eq!(export(&[], Path::new("/x")), "#EXTM3U\n");
    }

    #[test]
    fn paths_under_the_base_dir_become_relative() {
        let t = track("/music/rock/a.flac", "", "Song", 1);
        let text = export(&[&t], Path::new("/music"));
        assert!(text.ends_with(&format!("{}\n", Path::new("rock").join("a.flac").display())));
    }

    #[test]
    fn paths_outside_the_base_dir_stay_absolute() {
        let t = track("/music/a.flac", "", "Song", 1);
        assert!(export(&[&t], Path::new("/other")).ends_with("/music/a.flac\n"));
        // A sibling directory sharing a prefix is not "beneath" it.
        assert!(export(&[&t], Path::new("/mus")).ends_with("/music/a.flac\n"));
    }

    #[test]
    fn falls_back_to_the_file_name_and_flattens_newlines() {
        let untitled = track("/m/no tags.mp3", "", "", 0);
        let multi = track("/m/b.mp3", "A\nB", "T\r\nU", 2);
        let text = export(&[&untitled, &multi], Path::new("/x"));
        assert!(text.contains("#EXTINF:0,no tags\n"));
        assert!(text.contains("#EXTINF:2,A B - T  U\n"));
    }

    #[test]
    fn unicode_is_preserved() {
        let t = track("/m/日本.flac", "アーティスト", "曲", 3);
        let text = export(&[&t], Path::new("/x"));
        assert!(text.contains("アーティスト - 曲"));
        assert!(text.contains("/m/日本.flac"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_compare_case_insensitively() {
        let t = track(r"C:\Music\a.flac", "", "S", 1);
        assert!(export(&[&t], Path::new(r"c:\music")).ends_with("a.flac\n"));
        assert!(!export(&[&t], Path::new(r"c:\music")).contains("Music"));
    }
}
