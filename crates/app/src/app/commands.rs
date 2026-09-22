//! Applies queued [`Command`]s to the player, resolving library track ids
//! to filesystem paths where needed. Split out of `app/mod.rs` since it's a
//! distinct responsibility (translating UI intent into player calls) from
//! the `App`/`eframe::App` wiring itself.

use std::path::PathBuf;

use crate::library_api::LibraryDataSource;
use crate::player_api::{PlayerApi, RepeatMode};
use crate::state::{AppState, Command};

/// Applies a frame's queued library commands. `state.library_folders` is the
/// already-updated folder list, so a batch of adds/removes results in a
/// single [`LibraryDataSource::set_folders`] call (and one scan). Folders
/// added from an off-thread picker (#69) are also appended to the state here,
/// so they are persisted with the rest of the config.
pub(super) fn apply_library_commands(
    library: &mut dyn LibraryDataSource,
    state: &mut AppState,
    commands: &[Command],
) {
    let mut folders_changed = false;
    let mut rescan = false;
    let mut cancel = false;
    for cmd in commands {
        match cmd {
            Command::LibraryAddFolder(_) | Command::LibraryRemoveFolder(_) => {
                folders_changed = true
            }
            Command::LibraryRescan => rescan = true,
            Command::LibraryCancelScan => cancel = true,
            _ => {}
        }
    }

    for folder in library.folders() {
        let path = PathBuf::from(&folder.path);
        if !state.library_folders.contains(&path) {
            state.library_folders.push(path);
            folders_changed = true;
        }
    }

    if folders_changed {
        library.set_folders(&state.library_folders);
    }
    if rescan {
        library.rescan();
    }
    if cancel {
        library.cancel_scan();
    }
}

pub(super) fn apply_player_command(
    player: &mut dyn PlayerApi,
    library: &dyn LibraryDataSource,
    cmd: &Command,
) {
    match cmd {
        Command::PlayerPlayPause => player.play_pause(),
        Command::PlayerStop => player.stop(),
        Command::PlayerNext => player.next(),
        Command::PlayerPrevious => player.previous(),
        Command::PlayerSeek(pos) => player.seek(*pos),
        Command::PlayerSetVolume(v) => player.set_volume(*v),
        Command::PlayerToggleRepeat => player.set_repeat_mode(next_repeat(player.repeat_mode())),
        Command::PlayerToggleShuffle => player.set_shuffle(!player.shuffle()),
        Command::PlayerQueueJump(index) => player.queue_jump(*index),
        Command::PlayerQueueRemove(index) => player.queue_remove(*index),
        Command::PlayTrack(id) => {
            if let Some(path) = track_path(library, *id) {
                player.replace_and_play(std::slice::from_ref(&path), 0);
            }
        }
        Command::PlayAlbum(ids) => {
            let paths: Vec<PathBuf> = ids
                .iter()
                .filter_map(|id| track_path(library, *id))
                .collect();
            if !paths.is_empty() {
                player.replace_and_play(&paths, 0);
            }
        }
        Command::ShuffleScope { ids, label } => {
            let paths: Vec<PathBuf> = ids
                .iter()
                .filter_map(|id| track_path(library, *id))
                .collect();
            if !paths.is_empty() {
                player.play_shuffled(&paths, label);
            }
        }
        Command::PlayTrackNext(id) => {
            if let Some(path) = track_path(library, *id) {
                player.play_next(&path);
            }
        }
        Command::QueueTrack(id) => {
            if let Some(path) = track_path(library, *id) {
                player.enqueue(&path);
            }
        }
        _ => {}
    }
}

/// Resolves a library track id (as sent by the track table's context menu,
/// #11) to a filesystem path the player can open.
fn track_path(library: &dyn LibraryDataSource, id: u64) -> Option<PathBuf> {
    library
        .tracks()
        .iter()
        .find(|t| t.id == id)
        .map(|t| PathBuf::from(&t.path))
}

fn next_repeat(mode: RepeatMode) -> RepeatMode {
    mode.next()
}
