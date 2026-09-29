//! In-memory playlists for [`MockLibrary`](super::MockLibrary) (#473).

use emusic_library::reordered;

use crate::library_api::{PlaylistInfo, PlaylistItem};

/// Playlists held in memory, with the same semantics as the SQLite store.
#[derive(Default)]
pub(super) struct MockPlaylists {
    infos: Vec<PlaylistInfo>,
    items: Vec<Vec<PlaylistItem>>,
    next_id: u64,
    next_entry: i64,
}

impl MockPlaylists {
    pub(super) fn infos(&self) -> &[PlaylistInfo] {
        &self.infos
    }

    /// Seeds a few representative playlists from `tracks`, so the navigator's
    /// PLAYLISTS section and the Playlist view have content in `--mock` runs
    /// and screenshots (#476). A no-op for an empty library.
    pub(super) fn seed_demo(&mut self, tracks: &[u64]) {
        if tracks.is_empty() {
            return;
        }
        let pick = |index: usize| tracks[index % tracks.len()];
        let road = self.create("Road trip");
        self.add(road, &[pick(0), pick(3), pick(7), pick(12)]);
        let focus = self.create("Focus");
        self.add(focus, &[pick(1), pick(2), pick(5)]);
        let favourites = self.create("Favourites mix");
        self.add(favourites, &[pick(9), pick(0), pick(4)]);
    }

    pub(super) fn items(&self, id: u64) -> &[PlaylistItem] {
        self.index(id).map_or(&[], |i| &self.items[i])
    }

    pub(super) fn create(&mut self, name: &str) -> u64 {
        self.next_id += 1;
        self.infos.push(PlaylistInfo {
            id: self.next_id,
            name: name.to_owned(),
            track_count: 0,
        });
        self.items.push(Vec::new());
        self.next_id
    }

    pub(super) fn rename(&mut self, id: u64, name: &str) {
        if let Some(i) = self.index(id) {
            self.infos[i].name = name.to_owned();
        }
    }

    pub(super) fn delete(&mut self, id: u64) {
        if let Some(i) = self.index(id) {
            self.infos.remove(i);
            self.items.remove(i);
        }
    }

    pub(super) fn add(&mut self, id: u64, tracks: &[u64]) {
        let Some(i) = self.index(id) else { return };
        for &track_id in tracks {
            self.next_entry += 1;
            self.items[i].push(PlaylistItem {
                entry_id: self.next_entry,
                track_id,
            });
        }
        self.infos[i].track_count = self.items[i].len();
    }

    pub(super) fn remove(&mut self, id: u64, entries: &[i64]) {
        let Some(i) = self.index(id) else { return };
        self.items[i].retain(|item| !entries.contains(&item.entry_id));
        self.infos[i].track_count = self.items[i].len();
    }

    pub(super) fn relocate(&mut self, id: u64, entries: &[i64], to: usize) {
        let Some(i) = self.index(id) else { return };
        let current: Vec<i64> = self.items[i].iter().map(|item| item.entry_id).collect();
        let order = reordered(&current, entries, to);
        let old = std::mem::take(&mut self.items[i]);
        self.items[i] = order
            .iter()
            .filter_map(|entry| old.iter().find(|item| item.entry_id == *entry).copied())
            .collect();
    }

    fn index(&self, id: u64) -> Option<usize> {
        self.infos.iter().position(|info| info.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(playlists: &MockPlaylists, id: u64) -> Vec<u64> {
        playlists.items(id).iter().map(|i| i.track_id).collect()
    }

    #[test]
    fn add_remove_and_count() {
        let mut p = MockPlaylists::default();
        let id = p.create("Mix");
        p.add(id, &[5, 6, 5]);
        assert_eq!(tracks(&p, id), [5, 6, 5]);
        assert_eq!(p.infos()[0].track_count, 3);

        let first = p.items(id)[0].entry_id;
        p.remove(id, &[first]);
        assert_eq!(tracks(&p, id), [6, 5]);
        assert_eq!(p.infos()[0].track_count, 2);
    }

    #[test]
    fn relocate_matches_the_store_semantics() {
        let mut p = MockPlaylists::default();
        let id = p.create("Mix");
        p.add(id, &[1, 2, 3, 4]);
        let last = p.items(id)[3].entry_id;
        p.relocate(id, &[last], 0);
        assert_eq!(tracks(&p, id), [4, 1, 2, 3]);
    }

    #[test]
    fn unknown_playlists_are_ignored() {
        let mut p = MockPlaylists::default();
        p.add(9, &[1]);
        p.rename(9, "x");
        p.delete(9);
        assert!(p.infos().is_empty());
        assert!(p.items(9).is_empty());
    }
}
