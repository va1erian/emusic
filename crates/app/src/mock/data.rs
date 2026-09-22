//! Seeded fake-data generator: ~2000 tracks, ~80 artists, ~200 albums, ~15
//! genres, nested folders, a mix of formats (including tracker modules),
//! some missing tags, unicode/CJK names, and long titles.

use std::time::Duration;

use rand::Rng;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;

use crate::library_api::{
    AlbumInfo, ArtistInfo, DirNodeInfo, FolderInfo, HistoryEntry, TrackInfo, format_minutes_ago,
};

use super::dirs;
use super::generators::*;

/// Fixed seed so `emusic --mock` and `emusic-shot` render byte-identical
/// output across runs (required for snapshot tests).
const SEED: u64 = 0xE51C_2024;

const GENRES: &[&str] = &[
    "Rock",
    "Pop",
    "Jazz",
    "Classical",
    "Electronic",
    "Hip-Hop",
    "Metal",
    "Folk",
    "Blues",
    "Reggae",
    "Chiptune",
    "Ambient",
    "Soundtrack",
    "Demoscene",
    "Trance",
];

const FORMATS: &[&str] = &["mp3", "flac", "ogg", "m4a", "wav", "xm", "it", "mod", "s3m"];
const TRACKER_FORMATS: &[&str] = &["xm", "it", "mod", "s3m"];

/// Name fragments used to build pronounceable-ish fake artist/album/track
/// names, including some unicode/CJK entries so text-rendering and font
/// fallback get exercised.
const NAME_WORDS: &[&str] = &[
    "Crimson",
    "Velvet",
    "Neon",
    "Silent",
    "Broken",
    "Electric",
    "Paper",
    "Midnight",
    "Golden",
    "Static",
    "Hollow",
    "Wandering",
    "Iron",
    "Glass",
    "Faded",
    "夜の街",
    "桜",
    "雨音",
    "은하수",
    "여름밤",
    "Étoile",
    "Über",
    "Señorita",
    "Café",
    "Nørdic",
];
const NAME_NOUNS: &[&str] = &[
    "Wolves", "Horizon", "Echo", "Garden", "Machine", "River", "Dream", "Signal", "Ashes", "Tides",
    "Circuit", "Bloom", "Shadow", "Voyage", "Lantern", "楽団", "回廊", "물결", "여행", "Rêverie",
];

/// Everything the mock library needs, pre-generated once.
pub struct GeneratedLibrary {
    pub tracks: Vec<TrackInfo>,
    pub albums: Vec<AlbumInfo>,
    pub artists: Vec<ArtistInfo>,
    pub genres: Vec<String>,
    pub folders: Vec<FolderInfo>,
    pub dirs: Vec<DirNodeInfo>,
    pub history: Vec<HistoryEntry>,
    pub most_played: Vec<TrackInfo>,
}

pub fn generate() -> GeneratedLibrary {
    let mut rng = ChaCha8Rng::seed_from_u64(SEED);

    let artist_names: Vec<String> = (0..80)
        .map(|i| person_name(NAME_WORDS, NAME_NOUNS, &mut rng, i))
        .collect();
    let genres: Vec<String> = GENRES.iter().map(|s| s.to_string()).collect();

    let mut albums = Vec::new();
    for i in 0..200u32 {
        let artist = artist_names.choose(&mut rng).unwrap().clone();
        albums.push(AlbumInfo {
            name: phrase(NAME_WORDS, NAME_NOUNS, &mut rng, i),
            artist,
            year: rng.gen_bool(0.9).then(|| rng.gen_range(1975..=2025)),
            track_count: 0, // filled in below
        });
    }

    let folders: Vec<String> = build_folders(&mut rng, &artist_names);

    let mut tracks = Vec::with_capacity(2000);
    for id in 0..2000u64 {
        let album_idx = rng.gen_range(0..albums.len());
        let format = if rng.gen_bool(0.06) {
            *TRACKER_FORMATS.choose(&mut rng).unwrap()
        } else {
            *FORMATS
                .iter()
                .filter(|f| !TRACKER_FORMATS.contains(f))
                .collect::<Vec<_>>()
                .choose(&mut rng)
                .unwrap()
        };
        let is_tracker = TRACKER_FORMATS.contains(&format);
        let missing_tags = rng.gen_bool(0.05);

        let title = if is_tracker {
            // Tracker modules often carry their "title" as the module
            // message / an instrument-name-derived string.
            tracker_title(&mut rng, NAME_WORDS, NAME_NOUNS)
        } else if rng.gen_bool(0.03) {
            long_title(&mut rng, NAME_WORDS)
        } else {
            phrase(NAME_WORDS, NAME_NOUNS, &mut rng, id as u32)
        };

        let album = &albums[album_idx];
        let folder = folders.choose(&mut rng).unwrap().clone();
        let play_count = weighted_play_count(&mut rng);

        let (codec, bitrate, sample_rate, bit_depth, channels) =
            stream_metadata(format, TRACKER_FORMATS, &mut rng);

        tracks.push(TrackInfo {
            id,
            title: if missing_tags { String::new() } else { title },
            artist: if missing_tags {
                String::new()
            } else {
                album.artist.clone()
            },
            album: album.name.clone(),
            genre: genres.choose(&mut rng).unwrap().clone(),
            track_no: (!missing_tags).then(|| rng.gen_range(1..=18)),
            year: album.year,
            disc_no: (!missing_tags && rng.gen_bool(0.15)).then(|| rng.gen_range(1..=2)),
            duration: Duration::from_secs(rng.gen_range(45..=420)),
            path: format!("{folder}/{:03}.{format}", id % 1000),
            format: format.to_string(),
            codec,
            bitrate,
            sample_rate,
            bit_depth,
            channels,
            play_count,
            last_played_minutes_ago: (play_count > 0).then(|| *minutes_ago(&mut rng)),
        });
    }

    for album in &mut albums {
        album.track_count = tracks.iter().filter(|t| t.album == album.name).count();
    }
    albums.retain(|a| a.track_count > 0);

    let artists = artist_names
        .into_iter()
        .map(|name| {
            let their_tracks: Vec<_> = tracks.iter().filter(|t| t.artist == name).collect();
            ArtistInfo {
                track_count: their_tracks.len(),
                album_count: albums.iter().filter(|a| a.artist == name).count(),
                name,
            }
        })
        .filter(|a| a.track_count > 0)
        .collect();

    let folders = folders
        .into_iter()
        .map(|path| {
            let track_count = tracks.iter().filter(|t| t.path.starts_with(&path)).count();
            FolderInfo { path, track_count }
        })
        .collect();

    let dirs = dirs::build(&tracks);

    let mut most_played: Vec<TrackInfo> = tracks.clone();
    most_played.sort_by_key(|t| std::cmp::Reverse(t.play_count));
    most_played.truncate(50);

    let history = build_history(&mut rng, &tracks);

    GeneratedLibrary {
        tracks,
        albums,
        artists,
        genres,
        folders,
        dirs,
        history,
        most_played,
    }
}

fn build_history(rng: &mut ChaCha8Rng, tracks: &[TrackInfo]) -> Vec<HistoryEntry> {
    (0..60)
        .filter_map(|_| {
            let track = tracks.choose(rng)?;
            let mins = *minutes_ago(rng);
            Some(HistoryEntry {
                track_title: track.title.clone(),
                artist: track.artist.clone(),
                played_at: format_minutes_ago(mins),
            })
        })
        .collect()
}
