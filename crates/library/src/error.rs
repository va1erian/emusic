use std::path::PathBuf;

/// Errors returned by the `emusic-library` store.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    /// Underlying SQLite error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Could not determine or create the default database directory.
    #[error("could not resolve library data directory")]
    NoDataDir,

    /// Failed to create the database's parent directory.
    #[error("could not create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The database's `user_version` is newer than this build of emusic
    /// knows how to handle.
    #[error(
        "library database schema version {found} is newer than the {supported} supported by this build"
    )]
    UnsupportedSchemaVersion { found: i64, supported: i64 },
}

/// Convenience alias for results returned by this crate.
pub type Result<T> = std::result::Result<T, LibraryError>;
