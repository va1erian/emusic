//! Playlist-view model (#473): the shared track table over one playlist's
//! entries, kept in the playlist's own order, plus the commands the view
//! emits (remove, reorder, play). The entries come from the backend
//! ([`LibraryDataSource::playlist_items`]); rendering stays in the frontends.

use std::collections::HashMap;

use crate::library_api::{LibraryDataSource, PlaylistItem, TrackInfo};
use crate::state::Command;
use crate::views::track_table::TrackTable;
use crate::views::{Commands, Ctx};

/// A playlist's entries paired with the library tracks they refer to, in
/// playlist order. Entries whose track is not in the current library
/// snapshot (e.g. hidden by server-only mode) are left out of both lists,
/// which therefore stay index-aligned.
pub struct Resolved<'a> {
    pub items: Vec<PlaylistItem>,
    pub tracks: Vec<&'a TrackInfo>,
}

/// Resolves playlist `id` against the library.
pub fn resolve(library: &dyn LibraryDataSource, id: u64) -> Resolved<'_> {
    let entries = library.playlist_items(id);
    if entries.is_empty() {
        return Resolved {
            items: Vec::new(),
            tracks: Vec::new(),
        };
    }
    let by_id: HashMap<u64, &TrackInfo> = library
        .tracks()
        .iter()
        .map(|track| (track.id, track))
        .collect();
    let mut resolved = Resolved {
        items: Vec::with_capacity(entries.len()),
        tracks: Vec::with_capacity(entries.len()),
    };
    for item in entries {
        if let Some(track) = by_id.get(&item.track_id) {
            resolved.items.push(*item);
            resolved.tracks.push(track);
        }
    }
    resolved
}

/// Persistent Playlist-view state: the track table plus the entries it shows.
#[derive(Debug, Default)]
pub struct PlaylistView {
    /// The track table (sort + selection). Reordering is only meaningful in
    /// playlist order, i.e. with no sort column active.
    pub table: TrackTable,
    /// The entries the table shows, in playlist order.
    items: Vec<PlaylistItem>,
    /// Bumped whenever the entries change.
    revision: u64,
}

impl PlaylistView {
    /// Adopts the resolved entries and refreshes the table. `cx.tracks` must
    /// be the [`Resolved::tracks`] that go with `items`.
    ///
    /// Bumps the revision when the entries changed; an unchanged playlist
    /// costs one comparison per frame.
    pub fn refresh(&mut self, items: &[PlaylistItem], cx: &Ctx) {
        if self.items != items {
            self.items = items.to_vec();
            self.revision += 1;
        }
        self.table.refresh(cx);
    }

    /// The entries shown, in playlist order.
    pub fn items(&self) -> &[PlaylistItem] {
        &self.items
    }

    /// The number of entries shown.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// The header count text, e.g. `"5 tracks"`.
    pub fn count_label(&self) -> String {
        match self.items.len() {
            1 => "1 track".to_owned(),
            n => format!("{n} tracks"),
        }
    }

    /// The revision counter, bumped whenever the entries change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether the rows can be reordered: only in playlist order, since a
    /// sorted table's row positions are not playlist positions.
    pub fn can_reorder(&self) -> bool {
        self.table.sort.key.is_none()
    }

    /// The entries whose track is selected, in playlist order.
    ///
    /// Selection is keyed by track id, so selecting a track that appears
    /// twice selects both of its entries.
    pub fn selected_entries(&self) -> Vec<i64> {
        self.items
            .iter()
            .filter(|item| self.table.selection.is_selected(item.track_id))
            .map(|item| item.entry_id)
            .collect()
    }

    /// Queues removal of the selected entries from playlist `id` (the Delete
    /// key and the context menu's Remove).
    pub fn remove_selected(&self, id: u64, out: &mut Commands) {
        let entries = self.selected_entries();
        if !entries.is_empty() {
            out.push(Command::RemoveFromPlaylist { id, entries });
        }
    }

    /// Queues moving the entries at display `rows` so they land together at
    /// display index `to` (an insertion point: `0..=count`). Does nothing
    /// while a sort is active, or for rows out of range.
    ///
    /// `all` is the playlist's complete entry list
    /// ([`LibraryDataSource::playlist_items`]): display rows skip entries whose
    /// track is unavailable, so the display index is translated to the
    /// matching position in the full list, which is what the backend expects.
    pub fn move_rows(
        &self,
        id: u64,
        rows: &[usize],
        to: usize,
        all: &[PlaylistItem],
        out: &mut Commands,
    ) {
        if !self.can_reorder() {
            return;
        }
        let entries: Vec<i64> = rows
            .iter()
            .filter_map(|&row| self.items.get(row))
            .map(|item| item.entry_id)
            .collect();
        if entries.is_empty() {
            return;
        }
        let to = self
            .items
            .get(to)
            .and_then(|target| all.iter().position(|item| item.entry_id == target.entry_id))
            .unwrap_or(all.len());
        out.push(Command::MoveInPlaylist { id, entries, to });
    }

    /// Queues playing the whole playlist from the start, or shuffled.
    pub fn play(&self, id: u64, shuffle: bool, out: &mut Commands) {
        out.push(Command::PlayPlaylist { id, shuffle });
    }
}

#[cfg(test)]
mod tests;
