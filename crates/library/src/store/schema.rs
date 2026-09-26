//! Schema migrations, applied in order against `PRAGMA user_version`.
//!
//! Each entry is the SQL executed to go from version `i` to `i + 1`
//! (1-indexed: `MIGRATIONS[0]` takes the database from version 0 to 1).
//! Migrations are additive only: once released, an entry must never be
//! edited — add a new one instead.

/// The schema version this build of `emusic-library` expects.
pub const CURRENT_VERSION: i64 = 6;

pub const MIGRATIONS: &[&str] = &[
    // v1: initial schema.
    r"
    CREATE TABLE tracks (
        id              INTEGER PRIMARY KEY,
        path            TEXT NOT NULL UNIQUE,
        dir             TEXT NOT NULL,
        filename        TEXT NOT NULL,
        ext             TEXT NOT NULL,
        size            INTEGER NOT NULL,
        mtime           INTEGER NOT NULL,
        kind            INTEGER NOT NULL,
        duration_ms     INTEGER NOT NULL,
        bitrate         INTEGER,
        sample_rate     INTEGER,
        channels        INTEGER,
        title           TEXT,
        artist          TEXT,
        album_artist    TEXT,
        album           TEXT,
        genre           TEXT,
        year            INTEGER,
        track_no        INTEGER,
        disc_no         INTEGER,
        composer        TEXT,
        comment         TEXT,
        art_source_kind INTEGER NOT NULL,
        art_source_path TEXT,
        added_at        INTEGER NOT NULL
    );
    CREATE INDEX idx_tracks_dir ON tracks(dir);
    CREATE INDEX idx_tracks_album_artist ON tracks(album_artist);

    CREATE TABLE plays (
        id                  INTEGER PRIMARY KEY,
        track_id            INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        played_at           INTEGER NOT NULL,
        duration_played_ms  INTEGER NOT NULL
    );
    CREATE INDEX idx_plays_track_id ON plays(track_id);
    CREATE INDEX idx_plays_played_at ON plays(played_at);

    CREATE TABLE track_stats (
        track_id        INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
        play_count      INTEGER NOT NULL DEFAULT 0,
        last_played_at  INTEGER
    );

    CREATE TABLE folders (
        id      INTEGER PRIMARY KEY,
        path    TEXT NOT NULL UNIQUE,
        enabled INTEGER NOT NULL DEFAULT 1
    );
    ",
    // v2: play/skip stats — a completion flag on each play row, and a
    // running skip count alongside the existing play count.
    r"
    ALTER TABLE plays ADD COLUMN completed INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE track_stats ADD COLUMN skip_count INTEGER NOT NULL DEFAULT 0;
    ",
    // v3: per-folder watch flag for file-system watching.
    r"
    ALTER TABLE folders ADD COLUMN watch INTEGER NOT NULL DEFAULT 0;
    ",
    // v4: per-track star (favorite) flag (#131).
    r"
    ALTER TABLE tracks ADD COLUMN starred INTEGER NOT NULL DEFAULT 0;
    ",
    // v5: a play is inserted when a track *starts* (so the History view can
    // show it live) and finalized when it stops; `finished` marks the rows
    // that have been finalized. Pre-existing rows are already finished.
    r"
    ALTER TABLE plays ADD COLUMN finished INTEGER NOT NULL DEFAULT 1;
    ",
    // v6: remote tracks (#391). A row sourced from an `emusic-server` carries
    // the owning server, the server track id and the sync version at which it
    // last changed; `remote_servers` remembers the last synced library version
    // per server for delta sync.
    r"
    ALTER TABLE tracks ADD COLUMN remote_server_id TEXT;
    ALTER TABLE tracks ADD COLUMN remote_track_id TEXT;
    ALTER TABLE tracks ADD COLUMN remote_sync_version INTEGER;
    CREATE INDEX idx_tracks_remote ON tracks(remote_server_id);

    CREATE TABLE remote_servers (
        server_id       TEXT PRIMARY KEY,
        since_version   INTEGER NOT NULL DEFAULT 0
    );
    ",
];
