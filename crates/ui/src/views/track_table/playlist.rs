//! Playlist-related actions on a track table's rows (#473).

use crate::state::Command;
use crate::views::{Commands, Ctx};

use super::TrackTable;

impl TrackTable {
    /// The tracks a context-menu action on `row` applies to: the whole
    /// selection, in display order, when `row`'s track is part of it, else
    /// just that row's track. Empty for an out-of-range row.
    pub fn context_track_ids(&self, row: usize, cx: &Ctx) -> Vec<u64> {
        let Some(track) = self.track_at(row, cx) else {
            return Vec::new();
        };
        if !self.selection.is_selected(track.id) {
            return vec![track.id];
        }
        self.order_ids(cx)
            .into_iter()
            .filter(|id| self.selection.is_selected(*id))
            .collect()
    }

    /// Queues adding [`TrackTable::context_track_ids`] to playlist `id`
    /// ("Add to playlist" in a row's context menu, and dropping the rows on a
    /// navigator playlist).
    pub fn add_to_playlist(&self, id: u64, row: usize, cx: &Ctx, out: &mut Commands) {
        let tracks = self.context_track_ids(row, cx);
        if !tracks.is_empty() {
            out.push(Command::AddToPlaylist { id, tracks });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::library_api::TrackInfo;
    use crate::views::track_table::selection::ClickModifiers;

    use super::*;

    fn tracks() -> Vec<TrackInfo> {
        (1..=4)
            .map(|id| TrackInfo {
                id,
                ..TrackInfo::default()
            })
            .collect()
    }

    fn select(table: &mut TrackTable, cx: &Ctx, rows: &[usize]) {
        let order = table.order_ids(cx);
        for (n, &row) in rows.iter().enumerate() {
            let mods = ClickModifiers {
                ctrl: n > 0,
                ..ClickModifiers::default()
            };
            table.selection.click(&order, row, order[row], mods);
        }
    }

    #[test]
    fn a_row_inside_the_selection_acts_on_the_whole_selection() {
        let data = tracks();
        let refs: Vec<&TrackInfo> = data.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);
        select(&mut table, &cx, &[3, 1]);

        assert_eq!(table.context_track_ids(1, &cx), vec![2, 4]);
        assert_eq!(table.context_track_ids(3, &cx), vec![2, 4]);
    }

    #[test]
    fn a_row_outside_the_selection_acts_on_itself_only() {
        let data = tracks();
        let refs: Vec<&TrackInfo> = data.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);
        select(&mut table, &cx, &[0]);

        assert_eq!(table.context_track_ids(2, &cx), vec![3]);
        assert!(table.context_track_ids(99, &cx).is_empty());
    }

    #[test]
    fn add_to_playlist_queues_the_context_tracks() {
        let data = tracks();
        let refs: Vec<&TrackInfo> = data.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);
        select(&mut table, &cx, &[0, 2]);

        let mut out = Commands::new();
        table.add_to_playlist(7, 2, &cx, &mut out);
        table.add_to_playlist(7, 99, &cx, &mut out);
        assert_eq!(
            out.into_vec(),
            vec![Command::AddToPlaylist {
                id: 7,
                tracks: vec![1, 3]
            }]
        );
    }
}
