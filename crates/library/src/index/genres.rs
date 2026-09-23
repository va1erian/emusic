//! Genre grouping for the library index.

use std::collections::HashMap;

use emusic_core::Track;

use super::natural_sort::natural_compare;

/// Stable identifier for a genre entry.
pub type GenreId = usize;

const UNKNOWN_GENRE: &str = "Unknown Genre";

/// A genre with its associated tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct Genre {
    /// Stable id of this genre within the current index revision.
    pub id: GenreId,
    /// Display genre name.
    pub name: String,
    /// Indices of tracks in [`super::LibraryIndex::tracks`] tagged with this
    /// genre.
    pub track_slots: Vec<usize>,
}

impl Genre {
    /// Number of tracks in this genre.
    pub fn track_count(&self) -> usize {
        self.track_slots.len()
    }
}

/// Internal accumulator used while building or updating the index.
#[derive(Debug, Default)]
pub struct Genres {
    by_key: HashMap<String, Accumulator>,
    next_id: GenreId,
}

#[derive(Debug)]
struct Accumulator {
    id: GenreId,
    display_name: String,
    track_slots: Vec<usize>,
}

impl Genres {
    /// Registers a track with each genre parsed from its genre tag.
    pub fn add(&mut self, track: &Track, slot: usize) {
        let names = genre_names(track);
        if names.is_empty() {
            self.add_to_genre(UNKNOWN_GENRE, slot);
            return;
        }
        for name in names {
            self.add_to_genre(&name, slot);
        }
    }

    /// Removes a track from each genre it was previously registered under.
    pub fn remove(&mut self, track: &Track, slot: usize) {
        let names = genre_names(track);
        if names.is_empty() {
            self.remove_from_genre(UNKNOWN_GENRE, slot);
            return;
        }
        for name in names {
            self.remove_from_genre(&name, slot);
        }
    }

    /// Returns genres sorted by natural-order name.
    pub fn build(mut self) -> Vec<Genre> {
        let mut genres: Vec<Genre> = self
            .by_key
            .drain()
            .map(|(_, acc)| Genre {
                id: acc.id,
                name: acc.display_name,
                track_slots: acc.track_slots,
            })
            .collect();
        genres.sort_by(|a, b| natural_compare(&a.name, &b.name));
        genres
    }

    /// Rebuilds from scratch when the index is fully reset.
    pub fn rebuild(&mut self, tracks: &[Option<Track>]) {
        self.by_key.clear();
        self.next_id = 0;
        for (slot, track) in tracks.iter().enumerate() {
            if let Some(track) = track {
                self.add(track, slot);
            }
        }
    }

    fn add_to_genre(&mut self, name: &str, slot: usize) {
        let key = name.to_lowercase();
        let entry = self.by_key.entry(key).or_insert_with(|| {
            let id = self.next_id;
            self.next_id += 1;
            Accumulator {
                id,
                display_name: name.to_string(),
                track_slots: Vec::new(),
            }
        });
        entry.track_slots.push(slot);
    }

    fn remove_from_genre(&mut self, name: &str, slot: usize) {
        let key = name.to_lowercase();
        if let Some(entry) = self.by_key.get_mut(&key) {
            let before = entry.track_slots.len();
            entry.track_slots.retain(|&s| s != slot);
            debug_assert!(
                entry.track_slots.len() < before,
                "slot {slot} not found in genre '{name}'"
            );
            if entry.track_slots.is_empty() {
                self.by_key.remove(&key);
            }
        }
    }
}

/// Parses a genre tag into individual genre names.
///
/// Genres are split on `;`, `/` and `,` and trimmed.
pub fn genre_names(track: &Track) -> Vec<String> {
    track
        .genre
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.split(&[';', '/', ','][..])
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use emusic_core::{Track, TrackId, TrackKind};

    use super::*;

    fn track(genre: Option<&str>) -> Track {
        Track {
            id: TrackId::UNASSIGNED,
            path: PathBuf::from(r"C:\music\t.flac"),
            dir: PathBuf::from(r"C:\music"),
            filename: "t.flac".to_string(),
            ext: "flac".to_string(),
            size: 1,
            mtime: 1,
            kind: TrackKind::Stream,
            duration_ms: 60_000,
            bitrate: None,
            sample_rate: None,
            channels: None,
            title: None,
            artist: None,
            album_artist: None,
            album: None,
            genre: genre.map(ToString::to_string),
            year: None,
            track_no: None,
            disc_no: None,
            composer: None,
            comment: None,
            art_source: emusic_core::ArtSource::None,
            added_at: 1,
            starred: false,
        }
    }

    #[test]
    fn splits_multiple_genres() {
        let mut genres = Genres::default();
        genres.add(&track(Some("Rock; Metal / Punk, Indie")), 0);
        let built = genres.build();
        assert_eq!(built.len(), 4);
        let names: Vec<_> = built.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"Rock"));
        assert!(names.contains(&"Metal"));
        assert!(names.contains(&"Punk"));
        assert!(names.contains(&"Indie"));
    }

    #[test]
    fn unknown_genre_when_empty() {
        let mut genres = Genres::default();
        genres.add(&track(None), 0);
        let built = genres.build();
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].name, UNKNOWN_GENRE);
    }

    #[test]
    fn groups_case_insensitive() {
        let mut genres = Genres::default();
        genres.add(&track(Some("Rock")), 0);
        genres.add(&track(Some("rock")), 1);
        let built = genres.build();
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].track_count(), 2);
    }

    #[test]
    fn remove_clears_empty_group() {
        let mut genres = Genres::default();
        let t = track(Some("Jazz"));
        genres.add(&t, 0);
        genres.remove(&t, 0);
        assert!(genres.build().is_empty());
    }
}
