use emusic_core::{PlayEvent, TrackId};
use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::error::Result;

/// Aggregated play statistics for a single track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackStats {
    pub track_id: TrackId,
    pub play_count: u32,
    pub last_played_at: Option<i64>,
}

impl Store {
    /// Records a play event and updates the track's aggregated stats.
    pub fn record_play(&mut self, event: &PlayEvent) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO plays (track_id, played_at, duration_played_ms) VALUES (?1, ?2, ?3)",
            params![event.track_id.0, event.played_at, event.duration_played_ms],
        )?;
        tx.execute(
            "INSERT INTO track_stats (track_id, play_count, last_played_at)
             VALUES (?1, 1, ?2)
             ON CONFLICT(track_id) DO UPDATE SET
                play_count = play_count + 1,
                last_played_at = excluded.last_played_at",
            params![event.track_id.0, event.played_at],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Fetches aggregated stats for a track, if it has ever been played.
    pub fn track_stats(&self, track_id: TrackId) -> Result<Option<TrackStats>> {
        let stats = self
            .conn
            .query_row(
                "SELECT track_id, play_count, last_played_at FROM track_stats WHERE track_id = ?1",
                params![track_id.0],
                |row| {
                    Ok(TrackStats {
                        track_id: TrackId(row.get(0)?),
                        play_count: row.get(1)?,
                        last_played_at: row.get(2)?,
                    })
                },
            )
            .optional()?;
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use emusic_core::{ArtSource, Track, TrackKind};

    use super::*;

    fn store_with_one_track() -> (Store, TrackId) {
        let mut store = Store::open_in_memory().unwrap();
        let mut tracks = vec![Track {
            id: TrackId::UNASSIGNED,
            path: PathBuf::from(r"C:\music\a.flac"),
            dir: PathBuf::from(r"C:\music"),
            filename: "a.flac".to_string(),
            ext: "flac".to_string(),
            size: 1_000,
            mtime: 1_700_000_000,
            kind: TrackKind::Stream,
            duration_ms: 200_000,
            bitrate: None,
            sample_rate: None,
            channels: None,
            title: None,
            artist: None,
            album_artist: None,
            album: None,
            genre: None,
            year: None,
            track_no: None,
            disc_no: None,
            composer: None,
            comment: None,
            art_source: ArtSource::None,
            added_at: 1_700_000_000,
        }];
        store.upsert_tracks(&mut tracks).unwrap();
        let track_id = tracks[0].id;
        (store, track_id)
    }

    #[test]
    fn record_play_creates_and_updates_stats() {
        let (mut store, track_id) = store_with_one_track();

        store
            .record_play(&PlayEvent::new(track_id, 1_000, 30_000))
            .unwrap();
        let stats = store.track_stats(track_id).unwrap().unwrap();
        assert_eq!(stats.play_count, 1);
        assert_eq!(stats.last_played_at, Some(1_000));

        store
            .record_play(&PlayEvent::new(track_id, 2_000, 30_000))
            .unwrap();
        let stats = store.track_stats(track_id).unwrap().unwrap();
        assert_eq!(stats.play_count, 2);
        assert_eq!(stats.last_played_at, Some(2_000));
    }

    #[test]
    fn track_stats_none_when_never_played() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.track_stats(TrackId(42)).unwrap().is_none());
    }
}
