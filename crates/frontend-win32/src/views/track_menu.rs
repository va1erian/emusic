//! The track table's context menu (#107, #476): the shared row actions plus
//! the "Add to playlist" submenu, built from the current playlists each time
//! it is opened (menus are immutable after construction).
//!
//! Kept out of [`super::track_table`] so that module stays focused on the list
//! control itself.

use emusic_ui::library_api::PlaylistInfo;
use win32ui::Menu;

use crate::app::Msg;
use crate::views::track_table::ContextAction;

/// Builds the track context menu. `remove_from_playlist` adds the Playlist
/// view's "Remove from playlist" item; the "Add to playlist" submenu is built
/// from `playlists`.
pub fn track_context_menu(playlists: &[PlaylistInfo], remove_from_playlist: bool) -> Menu<Msg> {
    let mut menu = Menu::new()
        .item("Play", None, || Msg::ContextAction(ContextAction::Play))
        .item("Play next", None, || {
            Msg::ContextAction(ContextAction::PlayNext)
        })
        .item("Add to queue", None, || {
            Msg::ContextAction(ContextAction::AddToQueue)
        })
        .submenu("Add to playlist", playlist_submenu(playlists))
        .separator()
        .item("Star / Unstar", None, || {
            Msg::ContextAction(ContextAction::ToggleStar)
        })
        .separator()
        .item("Open file location", None, || {
            Msg::ContextAction(ContextAction::OpenFileLocation)
        })
        .item("Copy path", None, || {
            Msg::ContextAction(ContextAction::CopyPath)
        })
        .separator()
        .item("Edit tags…", None, || {
            Msg::ContextAction(ContextAction::EditTags)
        })
        .item("Properties…", None, || {
            Msg::ContextAction(ContextAction::Properties)
        });
    if remove_from_playlist {
        menu = menu
            .separator()
            .item("Remove from playlist", None, || Msg::PlaylistRemoveSelected);
    }
    menu
}

/// The "Add to playlist" submenu: one item per playlist, or a disabled
/// placeholder when there are none.
fn playlist_submenu(playlists: &[PlaylistInfo]) -> Menu<Msg> {
    if playlists.is_empty() {
        return Menu::new().disabled_item("(no playlists)", None, || Msg::Noop);
    }
    let mut menu = Menu::new();
    for playlist in playlists {
        let id = playlist.id;
        menu = menu.item(playlist.name.clone(), None, move || {
            Msg::AddSelectionToPlaylist(id)
        });
    }
    menu
}
