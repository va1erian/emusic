//! Off-thread "save playlist as" picker (#473).
//!
//! Like [`crate::folder_picker`], the native dialog runs on its own thread so
//! the UI never blocks; the chosen file comes back through a channel the
//! shell drains each tick and turns into [`Command::ExportPlaylist`].
//!
//! [`Command::ExportPlaylist`]: crate::state::Command::ExportPlaylist

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Mutex, OnceLock};

use crate::file_picker::{self, Filter};

/// A finished save dialog: the playlist and where the user wants it written.
type Picked = (u64, PathBuf);

struct Channel {
    tx: Sender<Picked>,
    rx: Mutex<Receiver<Picked>>,
}

static CHANNEL: OnceLock<Channel> = OnceLock::new();

/// Installs the process-wide result channel. Called by the shell; later calls
/// are ignored.
pub fn init() {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = CHANNEL.set(Channel {
        tx,
        rx: Mutex::new(rx),
    });
}

/// Opens a save dialog for exporting playlist `id`, named `playlist_name`, on
/// a background thread. Returns immediately; a cancelled dialog delivers
/// nothing.
pub fn request(id: u64, playlist_name: &str) {
    let Some(channel) = CHANNEL.get() else {
        return;
    };
    let tx = channel.tx.clone();
    let file_name = default_file_name(playlist_name);
    std::thread::spawn(move || {
        let filter = Filter {
            name: "M3U playlist",
            extensions: &["m3u8", "m3u"],
        };
        let picked = file_picker::save_file(filter, &file_name);
        if let Some(path) = picked {
            let _ = tx.send((id, path));
        }
    });
}

/// The next export the user confirmed, if a dialog finished since the last
/// call.
pub fn try_recv() -> Option<(u64, PathBuf)> {
    let channel = CHANNEL.get()?;
    channel
        .rx
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .try_recv()
        .ok()
}

/// `<name>.m3u8` with characters Windows forbids in file names replaced by
/// `_` (`"New playlist"` stays as is; a blank name becomes `playlist`).
pub fn default_file_name(playlist_name: &str) -> String {
    let cleaned: String = playlist_name
        .trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let stem = cleaned.trim_end_matches('.');
    let stem = if stem.is_empty() { "playlist" } else { stem };
    format!("{stem}.m3u8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe_and_keep_the_playlist_name() {
        assert_eq!(default_file_name("Road trip"), "Road trip.m3u8");
        assert_eq!(default_file_name("AC/DC: Live?"), "AC_DC_ Live_.m3u8");
        assert_eq!(default_file_name("  "), "playlist.m3u8");
        assert_eq!(default_file_name("..."), "playlist.m3u8");
    }
}
