//! Sorting for the track table: click a header to sort by that column
//! (click again to reverse), with sensible tie-breaking secondary keys so
//! equal primary values still land in a stable, useful order.

use std::cmp::Ordering;

use crate::library_api::TrackInfo;

use super::columns::{self, ColumnId};

/// Current sort column/direction. `key: None` means "library order"
/// (insertion order, i.e. by id), matching the row's original position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SortState {
    pub key: Option<ColumnId>,
    pub ascending: bool,
}

impl SortState {
    /// Applies a header click: switches to sorting by `id` ascending, or
    /// flips direction if `id` is already the active sort column.
    pub fn toggle(&mut self, id: ColumnId) {
        if self.key == Some(id) {
            self.ascending = !self.ascending;
        } else {
            self.key = Some(id);
            self.ascending = true;
        }
    }

    /// Arrow glyph to draw next to the active column's header, if any.
    pub fn indicator(&self, id: ColumnId) -> Option<&'static str> {
        (self.key == Some(id)).then_some(if self.ascending { "▲" } else { "▼" })
    }
}

/// Returns indices into `tracks`, ordered per `sort`.
pub fn sorted_indices(tracks: &[&TrackInfo], sort: SortState) -> Vec<usize> {
    let mut order: Vec<usize> = (0..tracks.len()).collect();
    let Some(key) = sort.key else {
        return order;
    };
    order.sort_by(|&a, &b| {
        let ord = compare(tracks[a], tracks[b], key);
        if sort.ascending { ord } else { ord.reverse() }
    });
    order
}

fn compare(a: &TrackInfo, b: &TrackInfo, key: ColumnId) -> Ordering {
    match key {
        ColumnId::Title => title_key(a)
            .cmp(&title_key(b))
            .then_with(|| a.artist.to_lowercase().cmp(&b.artist.to_lowercase()))
            .then_with(|| a.track_no.cmp(&b.track_no)),
        ColumnId::Artist => a
            .artist
            .to_lowercase()
            .cmp(&b.artist.to_lowercase())
            .then_with(|| a.album.to_lowercase().cmp(&b.album.to_lowercase()))
            .then_with(|| a.track_no.cmp(&b.track_no)),
        ColumnId::Album => a
            .album
            .to_lowercase()
            .cmp(&b.album.to_lowercase())
            .then_with(|| a.track_no.cmp(&b.track_no))
            .then_with(|| title_key(a).cmp(&title_key(b))),
        ColumnId::Year => a
            .year
            .cmp(&b.year)
            .then_with(|| a.album.to_lowercase().cmp(&b.album.to_lowercase())),
        ColumnId::Genre => a
            .genre
            .cmp(&b.genre)
            .then_with(|| a.artist.to_lowercase().cmp(&b.artist.to_lowercase())),
        ColumnId::Time => a.duration.cmp(&b.duration),
        ColumnId::Format => a
            .format
            .cmp(&b.format)
            .then_with(|| title_key(a).cmp(&title_key(b))),
        ColumnId::Plays => a.play_count.cmp(&b.play_count),
        // Mock data only carries a coarse "minutes ago" number; never-played
        // tracks (`None`) sort after played ones regardless of direction.
        ColumnId::LastPlayed => match (a.last_played_minutes_ago, b.last_played_minutes_ago) {
            (Some(x), Some(y)) => y.cmp(&x), // fewer minutes ago = more recent = first
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
        ColumnId::File => a.path.cmp(&b.path),
    }
}

fn title_key(t: &TrackInfo) -> String {
    columns::title_text(t).to_lowercase()
}
