//! Off-thread folder picker (#69).
//!
//! `rfd`'s synchronous `pick_folder()` runs a modal dialog on the calling
//! thread; called straight from a view's sync it would block the UI thread and
//! Windows marks the window as "not responding". This module runs the dialog
//! on a dedicated thread and delivers the chosen path through a channel that
//! the shell drains each frame.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Mutex, OnceLock};

/// A folder-choice request on its way back to the UI thread.
enum PickResult {
    /// The user chose a folder.
    Picked(PathBuf),
    /// The dialog was cancelled or closed.
    Cancelled,
}

/// A channel of completed folder-pick results.
///
/// Stored in a process-wide slot because views only get `&mut AppState` (not
/// a handle to the shell). Only the UI thread calls [`try_recv`]; the mutex
/// exists because `OnceLock`'s contents must be `Sync` and the sender is
/// cloneable for any thread.
struct Channel {
    tx: Sender<PickResult>,
    rx: Mutex<Receiver<PickResult>>,
}

static CHANNEL: OnceLock<Channel> = OnceLock::new();

/// Installs the process-wide result channel. Call once from the shell; later
/// calls are ignored.
pub fn init() {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = CHANNEL.set(Channel {
        tx,
        rx: Mutex::new(rx),
    });
}

/// Opens the native folder picker on a background thread.
///
/// The dialog cannot block the caller's frame, so this returns immediately;
/// the chosen path comes back through the channel installed by [`init`].
/// Each call spawns one short-lived thread that exits when its dialog closes.
pub fn request() {
    let Some(channel) = CHANNEL.get() else {
        return;
    };
    let tx = channel.tx.clone();
    std::thread::spawn(move || {
        let result = match rfd::FileDialog::new().pick_folder() {
            Some(path) => PickResult::Picked(path),
            None => PickResult::Cancelled,
        };
        let _ = tx.send(result);
    });
}

/// Returns the next folder the user picked, if the shell has installed the
/// channel and a dialog finished since the last call.
pub fn try_recv() -> Option<PathBuf> {
    let channel = CHANNEL.get()?;
    let rx = channel
        .rx
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        match rx.try_recv() {
            Ok(PickResult::Picked(path)) => return Some(path),
            // Older cancelled dialogs are discarded so a stale result can
            // never mask the one the user actually picked.
            Ok(PickResult::Cancelled) => continue,
            Err(_) => return None,
        }
    }
}
