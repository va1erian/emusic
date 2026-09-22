//! Artist grouping for the library index.

use std::collections::HashMap;

use emusic_core::Track;

use super::natural_sort::natural_compare;

/// Stable identifier for an artist entry.
pub type ArtistId = usize;

/// An artist with its associated tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct Artist {
    /// Stable id of this artist within the current index revision.
    pub id: ArtistId,
    /// Display name (preserves the first-seen casing).
    pub name: String,
    /// Indices of tracks in [`super::LibraryIndex::tracks`] whose effective
    /// artist matches this artist.
    pub track_slots: Vec<usize>,
}

impl Artist {
    /// Number of tracks by this artist.
    pub fn track_count(&self) -> usize {
        self.track_slots.len()
    }
}

/// Internal accumulator used while building or updating the index.
#[derive(Debug, Default)]
pub struct Artists {
    /// Map from normalized (lowercase) artist name to accumulator.
    by_key: HashMap<String, Accumulator>,
    next_id: ArtistId,
}

#[derive(Debug)]
struct Accumulator {
    id: ArtistId,
    display_name: String,
    track_slots: Vec<usize>,
}

impl Artists {
    /// Registers a track with its effective artist.
    pub fn add(&mut self, track: &Track, slot: usize) {
        let name = effective_artist_name(track);
        let key = name.to_lowercase();
        let entry = self.by_key.entry(key).or_insert_with(|| {
            let id = self.next_id;
            self.next_id += 1;
            Accumulator {
                id,
                display_name: name,
                track_slots: Vec::new(),
            }
        });
        entry.track_slots.push(slot);
    }

    /// Removes a track from its artist grouping.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if the track's slot is not present in the
    /// expected artist. This should never happen for a consistent index.
    pub fn remove(&mut self, track: &Track, slot: usize) {
        let name = effective_artist_name(track);
        let key = name.to_lowercase();
        if let Some(entry) = self.by_key.get_mut(&key) {
            let before = entry.track_slots.len();
            entry.track_slots.retain(|&s| s != slot);
            debug_assert!(
                entry.track_slots.len() < before,
                "slot {slot} not found in artist '{name}'"
            );
            if entry.track_slots.is_empty() {
                self.by_key.remove(&key);
            }
        }
    }

    /// Returns artists sorted by natural-order name.
    pub fn build(mut self) -> Vec<Artist> {
        let mut artists: Vec<Artist> = self
            .by_key
            .drain()
            .map(|(_, acc)| Artist {
                id: acc.id,
                name: acc.display_name,
                track_slots: acc.track_slots,
            })
            .collect();
        artists.sort_by(|a, b| natural_compare(&a.name, &b.name));
        artists
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
}

/// Returns the effective artist name for grouping: album artist falls back to
/// track artist, then to a literal "Unknown Artist".
pub fn effective_artist_name(track: &Track) -> String {
    track
        .effective_album_artist()
        .filter(|s| !s.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "Unknown Artist".to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use emusic_core::{Track, TrackId, TrackKind};

    use super::*;

    fn track(artist: &str, album_artist: Option<&str>) -> Track {
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
            artist: Some(artist.to_string()),
            album_artist: album_artist.map(ToString::to_string),
            album: None,
            genre: None,
            year: None,
            track_no: None,
            disc_no: None,
            composer: None,
            comment: None,
            art_source: emusic_core::ArtSource::None,
            added_at: 1,
        }
    }

    #[test]
    fn groups_by_case_insensitive_name() {
        let mut artists = Artists::default();
        artists.add(&track("Pink Floyd", None), 0);
        artists.add(&track("pink floyd", None), 1);
        let built = artists.build();
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].track_count(), 2);
    }

    #[test]
    fn falls_back_to_track_artist() {
        let mut artists = Artists::default();
        artists.add(&track("Artist", None), 0);
        let built = artists.build();
        assert_eq!(built[0].name, "Artist");
    }

    #[test]
    fn prefers_album_artist() {
        let mut artists = Artists::default();
        artists.add(&track("Feat", Some("Album Artist")), 0);
        let built = artists.build();
        assert_eq!(built[0].name, "Album Artist");
    }

    #[test]
    fn sorts_naturally_and_ignores_the() {
        let mut artists = Artists::default();
        artists.add(&track("The Cure", None), 0);
        artists.add(&track("Abba", None), 1);
        artists.add(&track("Pink Floyd", None), 2);
        let built = artists.build();
        let names: Vec<_> = built.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Abba", "The Cure", "Pink Floyd"]);
    }

    #[test]
    fn remove_clears_empty_group() {
        let mut artists = Artists::default();
        let t = track("Solo", None);
        artists.add(&t, 0);
        artists.remove(&t, 0);
        assert!(artists.build().is_empty());
    }
}
