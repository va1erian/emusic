//! Playlist commands (#473): applies playlist edits to the library, resolves
//! "play playlist" into a queue command, exports M3U files, and keeps the
//! selected playlist and the navigator's playlist list consistent.

use std::path::Path;

use tracing::warn;

use crate::export_picker;
use crate::library_api::LibraryDataSource;
use crate::panels::navigator::new_playlist_name;
use crate::state::{AppState, Command, View};
use crate::views::playlist;

/// Applies a frame's queued playlist commands.
pub(super) fn apply_commands(
    library: &mut dyn LibraryDataSource,
    state: &mut AppState,
    commands: &[Command],
) {
    for cmd in commands {
        match cmd {
            Command::CreatePlaylist(name) => {
                let name = match name.trim() {
                    "" => new_playlist_name(library.playlists()),
                    name => name.to_owned(),
                };
                if let Some(id) = library.create_playlist(&name) {
                    state.view = View::Playlist;
                    state.selected_playlist = Some(id);
                }
            }
            Command::RenamePlaylist { id, name } => {
                let name = name.trim();
                if !name.is_empty() {
                    library.rename_playlist(*id, name);
                }
            }
            Command::DeletePlaylist(id) => {
                library.delete_playlist(*id);
                if state.selected_playlist == Some(*id) {
                    state.selected_playlist = None;
                    state.view = View::Music;
                }
            }
            Command::AddToPlaylist { id, tracks } => library.add_to_playlist(*id, tracks),
            Command::RemoveFromPlaylist { id, entries } => {
                library.remove_from_playlist(*id, entries)
            }
            Command::MoveInPlaylist { id, entries, to } => {
                library.move_in_playlist(*id, entries, *to)
            }
            Command::ExportPlaylistAs(id) => {
                if let Some(info) = library.playlists().iter().find(|info| info.id == *id) {
                    export_picker::request(*id, &info.name);
                }
            }
            Command::ExportPlaylist { id, path } => export(library, *id, path),
            _ => {}
        }
    }
}

/// Turns [`Command::PlayPlaylist`] into the queue command that plays it, or
/// `None` for any other command or an empty playlist.
pub(super) fn expand_play(cmd: &Command, library: &dyn LibraryDataSource) -> Option<Command> {
    let Command::PlayPlaylist { id, shuffle } = cmd else {
        return None;
    };
    let ids: Vec<u64> = playlist::resolve(library, *id)
        .tracks
        .iter()
        .map(|track| track.id)
        .collect();
    if ids.is_empty() {
        return None;
    }
    Some(if *shuffle {
        let label = library
            .playlists()
            .iter()
            .find(|playlist| playlist.id == *id)
            .map(|playlist| playlist.name.clone())
            .unwrap_or_default();
        Command::ShuffleScope { ids, label }
    } else {
        Command::PlayAlbum(ids)
    })
}

/// Falls back to the Music view when the selected playlist no longer exists
/// (deleted, or a stale id restored from the config) and mirrors the
/// library's playlists into the navigator model.
pub(super) fn sync(library: &dyn LibraryDataSource, state: &mut AppState) {
    let exists = |id: u64| library.playlists().iter().any(|playlist| playlist.id == id);
    if state.selected_playlist.is_some_and(|id| !exists(id)) {
        state.selected_playlist = None;
    }
    if state.view == View::Playlist && state.selected_playlist.is_none() {
        state.view = View::Music;
    }
    state
        .navigator
        .sync_playlists(library.playlists(), state.selected_playlist);
}

/// Writes playlist `id` to `path` as an M3U file, with track paths relative
/// to the file's folder where they sit beneath it.
fn export(library: &dyn LibraryDataSource, id: u64, path: &Path) {
    let resolved = playlist::resolve(library, id);
    let base = path.parent().unwrap_or_else(|| Path::new(""));
    let text = crate::m3u::export(&resolved.tracks, base);
    if let Err(err) = std::fs::write(path, text) {
        warn!(%err, path = %path.display(), "could not export playlist");
    }
}

#[cfg(test)]
mod tests;
