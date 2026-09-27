//! SQLite schema migrations for the server store.
//!
//! Migrations are applied in order against `PRAGMA user_version`, exactly like
//! `emusic-library`: entry `i` takes the database from version `i - 1` to `i`.
//! Once released an entry must never be edited — add a new one instead.

/// The schema version this build expects.
pub const CURRENT_VERSION: i64 = 1;

/// Ordered migration statements.
pub const MIGRATIONS: &[&str] = &[
    // v1: initial schema.
    r"
    CREATE TABLE meta (
        key     TEXT PRIMARY KEY,
        value   TEXT NOT NULL
    );

    CREATE TABLE devices (
        id          TEXT PRIMARY KEY,
        name        TEXT NOT NULL,
        public_key  TEXT NOT NULL,
        paired_at   INTEGER NOT NULL,
        last_seen   INTEGER,
        is_revoked  INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE pairing_codes (
        id          INTEGER PRIMARY KEY,
        code_hash   TEXT NOT NULL,
        created_at  INTEGER NOT NULL,
        expires_at  INTEGER NOT NULL,
        used        INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX idx_pairing_codes_expires ON pairing_codes(expires_at);

    CREATE TABLE tracks (
        id              TEXT PRIMARY KEY,
        root_index      INTEGER NOT NULL,
        relative_path   TEXT NOT NULL,
        format          TEXT NOT NULL,
        kind            TEXT NOT NULL,
        title           TEXT,
        artist          TEXT,
        album_artist    TEXT,
        album           TEXT,
        album_id        TEXT,
        genre           TEXT,
        year            INTEGER,
        track_no        INTEGER,
        disc_no         INTEGER,
        duration_secs   REAL,
        subtunes        INTEGER NOT NULL DEFAULT 1,
        channels        INTEGER,
        file_size       INTEGER NOT NULL,
        mtime           INTEGER NOT NULL,
        hash            TEXT NOT NULL,
        has_art         INTEGER NOT NULL DEFAULT 0,
        sync_version    INTEGER NOT NULL,
        added_at        INTEGER NOT NULL
    );
    CREATE UNIQUE INDEX idx_tracks_root_path ON tracks(root_index, relative_path);
    CREATE INDEX idx_tracks_sync_version ON tracks(sync_version);
    CREATE INDEX idx_tracks_album_id ON tracks(album_id);

    CREATE TABLE tombstones (
        track_id        TEXT PRIMARY KEY,
        sync_version    INTEGER NOT NULL,
        deleted_at      INTEGER NOT NULL
    );
    CREATE INDEX idx_tombstones_sync_version ON tombstones(sync_version);
    ",
];

/// Seed the monotonic library version used by delta sync.
pub const INITIAL_LIBRARY_VERSION: i64 = 0;

/// Key under which the library version is stored in `meta`.
pub const META_LIBRARY_VERSION: &str = "library_version";
