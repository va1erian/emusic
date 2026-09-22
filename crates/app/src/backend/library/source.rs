//! Internal snapshot of the library exposed to the UI.
//!
//! [`Snapshot`] is built from the SQLite store + in-memory index on a
//! background thread and swapped into [`LibraryBackend`](super::LibraryBackend)
//! each frame via [`super::Update`]. Keeping it separate from the backend
//! keeps the conversion code testable and the backend file focused on
//! threading.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use emusic_library::index::{Album, Artist, LibraryIndex};
use emusic_library::stats::PlayRecord;
use emusic_library::{Folder, Store, Track, TrackId, TrackKind, TrackStats};

use super::stats;

use crate::library_api::{
    AlbumInfo, ArtistInfo, FolderInfo, HistoryEntry, TrackInfo, format_minutes_ago,
};

/// A complete, UI-ready view of the library at one point in time.
#[derive(Debug, Default)]
pub(crate) struct Snapshot {
    pub tracks: Vec<TrackInfo>,
    pub albums: Vec<AlbumInfo>,
    pub artists: Vec<ArtistInfo>,
    pub genres: Vec<String>,
    pub folders: Vec<FolderInfo>,
    pub history: Vec<HistoryEntry>,
    pub most_played: Vec<TrackInfo>,
}

impl Snapshot {
    /// Builds a snapshot from every track in `store`, plus the configured
    /// `folders` and their per-folder track counts.
    pub(crate) fn from_store(store: &Store, folders: &[Folder]) -> anyhow::Result<Self> {
        let tracks = store.load_all_tracks()?;
        let stats = stats::load_all(store, &tracks)?;
        let index = LibraryIndex::build(tracks);
        Ok(Self::from_index(index, folders, &stats, store))
    }

    /// Builds a snapshot from an already-populated index. Used when the
    /// scanner has just written to the store and we reload the tracks.
    pub(crate) fn from_index(
        index: LibraryIndex,
        folders: &[Folder],
        stats: &HashMap<TrackId, TrackStats>,
        store: &Store,
    ) -> Self {
        let mut id_to_index = HashMap::with_capacity(index.track_count());
        let tracks: Vec<TrackInfo> = index
            .tracks()
            .enumerate()
            .map(|(i, track)| {
                id_to_index.insert(track.id.0 as u64, i);
                track_to_info(track, stats)
            })
            .collect();

        let albums: Vec<AlbumInfo> = index.albums().iter().map(album_to_info).collect();
        let artists = index
            .artists()
            .iter()
            .map(|artist| artist_to_info(artist, index.albums()))
            .collect();
        let genres = index.genres().iter().map(|g| g.name.clone()).collect();
        let folders = folders_to_info(folders, &index);
        let history = history_from_store(store, &id_to_index, &tracks).unwrap_or_default();
        let most_played = most_played_from_store(store, &id_to_index, &tracks).unwrap_or_default();

        Self {
            tracks,
            albums,
            artists,
            genres,
            folders,
            history,
            most_played,
        }
    }

    /// Applies a finished play to the in-memory stats without rebuilding the
    /// whole snapshot.
    ///
    /// Only reflects plays that hit a track already in this snapshot; the
    /// store (via the recorder) remains the source of truth, so the next
    /// full reload picks up anything missed here.
    pub(crate) fn record_play(&mut self, record: &PlayRecord) {
        let path = record.path.to_string_lossy();
        if let Some(track) = self.tracks.iter_mut().find(|t| t.path == path.as_ref()) {
            if record.completed {
                track.play_count += 1;
            }
            let minutes_ago = ((unix_now() - record.started_at).max(0) / 60) as u32;
            track.last_played_minutes_ago = Some(minutes_ago);
        }

        if !record.completed {
            return;
        }
        if let Some(entry) = self
            .most_played
            .iter_mut()
            .find(|t| t.path == path.as_ref())
        {
            entry.play_count += 1;
        }
    }
}

fn track_to_info(track: &Track, stats: &HashMap<TrackId, TrackStats>) -> TrackInfo {
    let stats = stats.get(&track.id);
    let play_count = stats.map_or(0, |s| s.play_count);
    let last_played_minutes_ago = stats.and_then(|s| {
        s.last_played_at.map(|ts| {
            let minutes = (unix_now() - ts).max(0) / 60;
            minutes as u32
        })
    });

    TrackInfo {
        id: track.id.0 as u64,
        title: track.display_title().to_string(),
        artist: track.artist.clone().unwrap_or_default(),
        album: track.album.clone().unwrap_or_default(),
        genre: track.genre.clone().unwrap_or_default(),
        track_no: track.track_no,
        year: track
            .year
            .and_then(|y| if y >= 0 { Some(y as u32) } else { None }),
        disc_no: track.disc_no,
        duration: Duration::from_millis(u64::from(track.duration_ms)),
        path: track.path.to_string_lossy().into_owned(),
        format: track.ext.clone(),
        codec: codec_for(&track.ext, track.kind),
        bitrate: track.bitrate,
        sample_rate: track.sample_rate,
        bit_depth: None,
        channels: track.channels,
        play_count,
        last_played_minutes_ago,
    }
}

fn codec_for(ext: &str, kind: TrackKind) -> String {
    match kind {
        TrackKind::Module => "Module".to_string(),
        TrackKind::Stream => ext.to_ascii_uppercase(),
    }
}

fn album_to_info(album: &Album) -> AlbumInfo {
    AlbumInfo {
        name: album.title.clone(),
        artist: album.artist.clone(),
        year: album
            .year
            .and_then(|y| if y >= 0 { Some(y as u32) } else { None }),
        track_count: album.track_count(),
    }
}

fn artist_to_info(artist: &Artist, albums: &[Album]) -> ArtistInfo {
    let album_count = albums
        .iter()
        .filter(|album| album.artist == artist.name)
        .count();
    ArtistInfo {
        name: artist.name.clone(),
        track_count: artist.track_count(),
        album_count,
    }
}

fn folders_to_info(folders: &[Folder], index: &LibraryIndex) -> Vec<FolderInfo> {
    let prefix_counts: HashMap<PathBuf, usize> =
        index.tracks().fold(HashMap::new(), |mut acc, track| {
            for folder in folders {
                if track.path.starts_with(&folder.path) {
                    *acc.entry(folder.path.clone()).or_insert(0) += 1;
                }
            }
            acc
        });

    folders
        .iter()
        .map(|folder| FolderInfo {
            path: folder.path.to_string_lossy().into_owned(),
            track_count: prefix_counts.get(&folder.path).copied().unwrap_or(0),
        })
        .collect()
}

fn history_from_store(
    store: &Store,
    id_to_index: &HashMap<u64, usize>,
    tracks: &[TrackInfo],
) -> anyhow::Result<Vec<HistoryEntry>> {
    const HISTORY_PAGE_SIZE: u32 = 60;
    let entries = store.play_history_page(HISTORY_PAGE_SIZE, 0)?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let track = id_to_index
                .get(&(entry.track_id.0 as u64))
                .map(|&i| &tracks[i])?;
            let minutes_ago = ((unix_now() - entry.played_at).max(0) / 60) as u32;
            Some(HistoryEntry {
                track_title: track.title.clone(),
                artist: track.artist.clone(),
                played_at: format_minutes_ago(minutes_ago),
            })
        })
        .collect())
}

fn most_played_from_store(
    store: &Store,
    id_to_index: &HashMap<u64, usize>,
    tracks: &[TrackInfo],
) -> anyhow::Result<Vec<TrackInfo>> {
    const MOST_PLAYED_LIMIT: u32 = 50;
    let entries = store.most_played_since(None, MOST_PLAYED_LIMIT)?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            id_to_index.get(&(entry.track_id.0 as u64)).map(|&i| {
                let mut track = tracks[i].clone();
                track.play_count = entry.play_count;
                track
            })
        })
        .collect())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
