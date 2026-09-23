//! In-memory library index with groupings.
//!
//! [`LibraryIndex`] stores all tracks in a slot map and maintains derived
//! views: artists, albums, genres and a directory tree. It supports building
//! from a full track list and applying incremental upserts/deletes.

use std::collections::HashMap;

use emusic_core::{Track, TrackId};

pub mod albums;
pub mod artists;
pub mod dirs;
pub mod genres;
pub mod natural_sort;
mod scopes;

pub use albums::{Album, AlbumId};
pub use artists::{Artist, ArtistId};
pub use dirs::DirNode;
pub use genres::{Genre, GenreId};
pub use scopes::ShuffleScope;

/// Aggregated totals across the whole library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Totals {
    /// Number of tracks in the library.
    pub track_count: usize,
    /// Sum of track durations in milliseconds.
    pub duration_ms: u64,
}

/// An in-memory index of the music library.
#[derive(Debug, Default)]
pub struct LibraryIndex {
    /// All tracks, indexed by slot. `None` entries are free slots.
    tracks: Vec<Option<Track>>,
    /// Map from track id to its slot in `tracks`.
    id_to_slot: HashMap<TrackId, usize>,
    /// Free slots that can be reused.
    free_slots: Vec<usize>,

    artists: artists::Artists,
    albums: albums::Albums,
    genres: genres::Genres,
    dirs: dirs::Dirs,

    artist_vec: Vec<Artist>,
    album_vec: Vec<Album>,
    genre_vec: Vec<Genre>,
    root_dir_vec: Vec<DirNode>,
    totals: Totals,
}

impl LibraryIndex {
    /// Creates an empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds an index from a complete list of tracks.
    ///
    /// Track ids are assumed to be assigned and unique. The input order is
    /// preserved as the slot order.
    pub fn build(tracks: Vec<Track>) -> Self {
        let mut index = Self::new();
        index.tracks.reserve(tracks.len());
        for track in tracks {
            let slot = index.tracks.len();
            index.id_to_slot.insert(track.id, slot);
            index.tracks.push(Some(track));
        }
        index.rebuild_accumulators_from_scratch();
        index.refresh_views();
        index
    }

    /// Applies a batch of upserts and deletes to the index.
    ///
    /// For each track in `upserts`, if the track id already exists its entry
    /// is updated; otherwise a new slot is allocated. Then each id in
    /// `deletes` is removed. Derived groupings are updated incrementally.
    pub fn apply(&mut self, upserts: Vec<Track>, deletes: &[TrackId]) {
        for id in deletes {
            self.remove(*id);
        }
        for track in upserts {
            self.upsert(track);
        }
        self.refresh_views();
    }

    /// Returns the track with the given id, if present.
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.id_to_slot
            .get(&id)
            .and_then(|&slot| self.tracks[slot].as_ref())
    }

    /// Returns all tracks in slot order.
    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.tracks.iter().filter_map(|t| t.as_ref())
    }

    /// Returns the starred (favorited) tracks, in slot order (#131).
    pub fn starred(&self) -> impl Iterator<Item = &Track> {
        self.tracks().filter(|track| track.starred)
    }

    /// Returns the number of tracks in the index.
    pub fn track_count(&self) -> usize {
        self.id_to_slot.len()
    }

    /// Returns the artist grouping.
    pub fn artists(&self) -> &[Artist] {
        &self.artist_vec
    }

    /// Returns the album grouping.
    pub fn albums(&self) -> &[Album] {
        &self.album_vec
    }

    /// Returns the genre grouping.
    pub fn genres(&self) -> &[Genre] {
        &self.genre_vec
    }

    /// Returns the root directory nodes.
    pub fn root_dirs(&self) -> &[DirNode] {
        &self.root_dir_vec
    }

    /// Returns library-wide totals.
    pub fn totals(&self) -> Totals {
        self.totals
    }

    fn upsert(&mut self, track: Track) {
        if let Some(&slot) = self.id_to_slot.get(&track.id) {
            if let Some(old) = self.tracks[slot].take() {
                self.artists.remove(&old, slot);
                self.albums.remove(&old, slot);
                self.genres.remove(&old, slot);
                self.dirs.remove(&old, slot);
            }
            self.tracks[slot] = Some(track.clone());
            self.artists.add(&track, slot);
            self.albums.add(&track, slot);
            self.genres.add(&track, slot);
            self.dirs.add(&track, slot);
        } else {
            let slot = self.free_slots.pop().unwrap_or_else(|| {
                let slot = self.tracks.len();
                self.tracks.push(None);
                slot
            });
            self.id_to_slot.insert(track.id, slot);
            self.tracks[slot] = Some(track.clone());
            self.artists.add(&track, slot);
            self.albums.add(&track, slot);
            self.genres.add(&track, slot);
            self.dirs.add(&track, slot);
        }
    }

    fn remove(&mut self, id: TrackId) {
        if let Some(&slot) = self.id_to_slot.get(&id) {
            if let Some(old) = self.tracks[slot].take() {
                self.artists.remove(&old, slot);
                self.albums.remove(&old, slot);
                self.genres.remove(&old, slot);
                self.dirs.remove(&old, slot);
            }
            self.id_to_slot.remove(&id);
            self.free_slots.push(slot);
        }
    }

    fn rebuild_accumulators_from_scratch(&mut self) {
        self.artists.rebuild(&self.tracks);
        self.albums.rebuild(&self.tracks);
        self.genres.rebuild(&self.tracks);
        self.dirs.rebuild(&self.tracks);
    }

    fn refresh_views(&mut self) {
        self.artist_vec = std::mem::take(&mut self.artists).build();
        self.album_vec = std::mem::take(&mut self.albums).build();
        self.genre_vec = std::mem::take(&mut self.genres).build();
        self.root_dir_vec = std::mem::take(&mut self.dirs).build();

        self.totals = self.tracks.iter().filter_map(|t| t.as_ref()).fold(
            Totals::default(),
            |mut acc, track| {
                acc.track_count += 1;
                acc.duration_ms += u64::from(track.duration_ms);
                acc
            },
        );
    }
}

#[cfg(test)]
mod tests;
