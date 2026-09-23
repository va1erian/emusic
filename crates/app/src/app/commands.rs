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
    let mut remove_history = None;
    let mut clear_history = false;
    let mut toggle_starred: Vec<u64> = Vec::new();
    for cmd in commands {
        match cmd {
            Command::LibraryAddFolder(_) | Command::LibraryRemoveFolder(_) => {
                folders_changed = true
            }
            Command::LibraryRescan => rescan = true,
            Command::LibraryCancelScan => cancel = true,
            Command::HistoryRemove(id) => remove_history = Some(*id),
            Command::HistoryClear => clear_history = true,
            Command::ToggleStarred(id) => toggle_starred.push(*id),
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
    if let Some(id) = remove_history {
        library.remove_history_entry(id);
    }
    if clear_history {
        library.clear_history();
    }
    for id in toggle_starred {
        if let Some(starred) = library
            .tracks()
            .iter()
            .find(|track| track.id == id)
            .map(|track| !track.starred)
        {
            library.set_starred(id, starred);
        }
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
        Command::PlayTrack { id, context } => {
            play_track_with_context(player, library, *id, context)
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
        Command::SetTrackerSettings(settings) => player.set_tracker_settings(settings),
        _ => {}
    }
}

/// Resolves `context` (falling back to just `id` when it's empty) to
/// filesystem paths and replaces the queue with them, starting at `id`'s
/// position — the shared logic behind [`Command::PlayTrack`] (#134), so the
/// queue actually has something for `next()`/`previous()` to walk through
/// instead of the one-entry queue that made them act like Stop.
///
/// Ids that no longer resolve to a track (stale context, e.g. a track
/// removed from the library between click and apply) are silently dropped;
/// `id`'s position is found in the resolved list, or defaults to the start
/// if `id` itself didn't resolve.
fn play_track_with_context(
    player: &mut dyn PlayerApi,
    library: &dyn LibraryDataSource,
    id: u64,
    context: &[u64],
) {
    let ids: &[u64] = if context.is_empty() {
        std::slice::from_ref(&id)
    } else {
        context
    };
    let resolved: Vec<(u64, PathBuf)> = ids
        .iter()
        .filter_map(|tid| track_path(library, *tid).map(|path| (*tid, path)))
        .collect();
    if resolved.is_empty() {
        return;
    }
    let start = resolved.iter().position(|(tid, _)| *tid == id).unwrap_or(0);
    let paths: Vec<PathBuf> = resolved.into_iter().map(|(_, path)| path).collect();
    player.replace_and_play(&paths, start);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockLibrary;

    #[test]
    fn toggle_starred_flips_the_track_flag() {
        let mut library = MockLibrary::new();
        let mut state = AppState::default();
        let id = library.tracks()[0].id;
        let before = library
            .tracks()
            .iter()
            .find(|track| track.id == id)
            .expect("first mock track exists")
            .starred;

        apply_library_commands(&mut library, &mut state, &[Command::ToggleStarred(id)]);

        let after = library
            .tracks()
            .iter()
            .find(|track| track.id == id)
            .expect("first mock track still exists")
            .starred;
        assert_eq!(after, !before);
        assert_eq!(
            library.starred_tracks().iter().any(|track| track.id == id),
            !before,
            "the starred set should follow the flag"
        );
    }
}
