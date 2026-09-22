//! Applies queued [`Command`]s to the player, resolving library track ids
//! to filesystem paths where needed. Split out of `app/mod.rs` since it's a
//! distinct responsibility (translating UI intent into player calls) from
//! the `App`/`eframe::App` wiring itself.

use std::path::PathBuf;

use crate::library_api::LibraryDataSource;
use crate::player_api::{PlayerApi, RepeatMode};
use crate::state::Command;

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
