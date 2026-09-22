//! Album grouping for the library index.

use std::collections::HashMap;

use emusic_core::Track;

use super::natural_sort::natural_compare;

/// Stable identifier for an album entry.
pub type AlbumId = usize;

const VARIOUS_ARTISTS: &str = "Various Artists";
const UNKNOWN_ARTIST: &str = "Unknown Artist";
const UNKNOWN_ALBUM: &str = "Unknown Album";

/// An album with aggregated metadata and its tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct Album {
    /// Stable id of this album within the current index revision.
    pub id: AlbumId,
    /// Display album title.
    pub title: String,
    /// Display album artist name.
    pub artist: String,
    /// Release year, if any track on the album has one.
    pub year: Option<i32>,
    /// Indices of tracks in [`super::LibraryIndex::tracks`] belonging to this
    /// album.
    pub track_slots: Vec<usize>,
    /// Sum of track durations in milliseconds.
    pub duration_ms: u64,
    /// Slot of a representative track to use for artwork.
    pub art_slot: Option<usize>,
}

impl Album {
    /// Number of tracks on the album.
    pub fn track_count(&self) -> usize {
        self.track_slots.len()
    }
}

/// Internal accumulator used while building or updating the index.
#[derive(Debug, Default)]
pub struct Albums {
    /// Albums keyed by (normalized artist, normalized title).
    by_key: HashMap<AlbumKey, Accumulator>,
    /// Per-title metadata used to decide the Various Artists promotion.
    titles: HashMap<String, TitleInfo>,
    /// Map from track slot to the album key it currently belongs to.
    slot_to_key: HashMap<usize, AlbumKey>,
    next_id: AlbumId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AlbumKey {
    artist: String,
    title: String,
}

#[derive(Debug, Default)]
struct TitleInfo {
    /// Number of tracks for this title.
    track_count: usize,
    /// Count of tracks per normalized contributing artist name.
    artist_counts: HashMap<String, usize>,
    /// Whether this title has been promoted to Various Artists.
    promoted: bool,
}

#[derive(Debug)]
struct Accumulator {
    id: AlbumId,
    display_artist: String,
    display_title: String,
    year: Option<i32>,
    track_slots: Vec<usize>,
    duration_ms: u64,
    /// Slots of tracks that have artwork, in insertion order.
    art_slots: Vec<usize>,
}

impl Albums {
    /// Registers a track with its album.
    pub fn add(&mut self, track: &Track, slot: usize) {
        let title = album_title(track);
        let title_lower = title.to_lowercase();
        let title_info = self.titles.entry(title_lower.clone()).or_default();
        title_info.track_count += 1;
        if let Some(artist) = track
            .artist
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            *title_info
                .artist_counts
                .entry(artist.to_lowercase())
                .or_insert(0) += 1;
        }

        let artist = if title_info.promoted {
            VARIOUS_ARTISTS.to_string()
        } else if title_info.artist_counts.len() > 3 {
            title_info.promoted = true;
            self.promote_title(&title, &title_lower);
            VARIOUS_ARTISTS.to_string()
        } else {
            initial_album_artist(track)
        };

        let key = AlbumKey::new(&artist, &title);
        self.slot_to_key.insert(slot, key.clone());

        let id = match self.by_key.get(&key) {
            Some(acc) => acc.id,
            None => {
                let id = self.next_id;
                self.next_id += 1;
                id
            }
        };

        let entry = self.by_key.entry(key).or_insert_with(|| Accumulator {
            id,
            display_artist: artist.clone(),
            display_title: title.clone(),
            year: None,
            track_slots: Vec::new(),
            duration_ms: 0,
            art_slots: Vec::new(),
        });

        entry.track_slots.push(slot);
        if entry.year.is_none() {
            entry.year = track.year;
        }
        entry.duration_ms += u64::from(track.duration_ms);
        if !matches!(track.art_source, emusic_core::ArtSource::None) {
            entry.art_slots.push(slot);
        }
    }

    /// Removes a track from its album grouping.
    pub fn remove(&mut self, track: &Track, slot: usize) {
        let key = match self.slot_to_key.remove(&slot) {
            Some(k) => k,
            None => return,
        };

        let Some(entry) = self.by_key.get_mut(&key) else {
            return;
        };

        let before = entry.track_slots.len();
        entry.track_slots.retain(|&s| s != slot);
        debug_assert!(
            entry.track_slots.len() < before,
            "slot {slot} not found in album '{key:?}'"
        );

        entry.duration_ms = entry
            .duration_ms
            .saturating_sub(u64::from(track.duration_ms));
        entry.art_slots.retain(|&s| s != slot);

        if entry.track_slots.is_empty() {
            self.by_key.remove(&key);
        }

        let title = album_title(track);
        let title_lower = title.to_lowercase();
        if let Some(title_info) = self.titles.get_mut(&title_lower) {
            title_info.track_count = title_info.track_count.saturating_sub(1);
            if let Some(artist) = track
                .artist
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let key_lower = artist.to_lowercase();
                if let Some(count) = title_info.artist_counts.get_mut(&key_lower) {
                    *count -= 1;
                    if *count == 0 {
                        title_info.artist_counts.remove(&key_lower);
                    }
                }
            }
            if title_info.track_count == 0 {
                self.titles.remove(&title_lower);
            }
        }
    }

    /// Returns albums sorted by artist then title in natural order.
    pub fn build(mut self) -> Vec<Album> {
        let mut albums: Vec<Album> = self
            .by_key
            .drain()
            .map(|(_, acc)| Album {
                id: acc.id,
                title: acc.display_title,
                artist: acc.display_artist,
                year: acc.year,
                track_slots: acc.track_slots,
                duration_ms: acc.duration_ms,
                art_slot: acc.art_slots.first().copied(),
            })
            .collect();
        albums.sort_by(|a, b| {
            natural_compare(&a.artist, &b.artist).then_with(|| natural_compare(&a.title, &b.title))
        });
        albums
    }

    /// Rebuilds from scratch when the index is fully reset.
    pub fn rebuild(&mut self, tracks: &[Option<Track>]) {
        self.by_key.clear();
        self.titles.clear();
        self.slot_to_key.clear();
        self.next_id = 0;
        for (slot, track) in tracks.iter().enumerate() {
            if let Some(track) = track {
                self.add(track, slot);
            }
        }
    }

    fn promote_title(&mut self, title: &str, title_lower: &str) {
        let va_key = AlbumKey::new(VARIOUS_ARTISTS, title);

        // Collect keys that belong to this title.
        let keys_to_merge: Vec<AlbumKey> = self
            .by_key
            .keys()
            .filter(|k| k.title == title_lower)
            .cloned()
            .collect();

        let mut merged = Accumulator {
            id: self.next_id,
            display_artist: VARIOUS_ARTISTS.to_string(),
            display_title: title.to_string(),
            year: None,
            track_slots: Vec::new(),
            duration_ms: 0,
            art_slots: Vec::new(),
        };
        self.next_id += 1;

        for key in keys_to_merge {
            let Some(acc) = self.by_key.remove(&key) else {
                continue;
            };
            if merged.year.is_none() {
                merged.year = acc.year;
            }
            merged.track_slots.extend(acc.track_slots.iter().copied());
            merged.duration_ms += acc.duration_ms;
            merged.art_slots.extend(acc.art_slots.iter().copied());
            for &slot in &acc.track_slots {
                self.slot_to_key.insert(slot, va_key.clone());
            }
        }

        if let Some(existing) = self.by_key.get_mut(&va_key) {
            existing
                .track_slots
                .extend(merged.track_slots.iter().copied());
            existing.duration_ms += merged.duration_ms;
            existing.art_slots.extend(merged.art_slots.iter().copied());
            if existing.year.is_none() {
                existing.year = merged.year;
            }
            for &slot in &merged.track_slots {
                self.slot_to_key.insert(slot, va_key.clone());
            }
        } else {
            self.by_key.insert(va_key.clone(), merged);
        }
    }
}

impl AlbumKey {
    fn new(artist: &str, title: &str) -> Self {
        Self {
            artist: artist.to_lowercase(),
            title: title.to_lowercase(),
        }
    }
}

fn initial_album_artist(track: &Track) -> String {
    track
        .effective_album_artist()
        .filter(|s| !s.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| UNKNOWN_ARTIST.to_string())
}

fn album_title(track: &Track) -> String {
    track
        .album
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| UNKNOWN_ALBUM.to_string())
}

#[cfg(test)]
mod tests;
