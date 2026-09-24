#![forbid(unsafe_code)]

//! Errors returned by the safe SID API.

use std::path::PathBuf;

use thiserror::Error;

/// Something went wrong loading or running a SID tune.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SidError {
    /// The file is shorter than the smallest valid PSID/RSID header.
    #[error("file is too small to be a PSID/RSID tune")]
    TooSmall,
    /// The first four bytes are neither `PSID` nor `RSID`.
    #[error("not a PSID/RSID file")]
    BadMagic,
    /// The header's declared size is impossible for the file.
    #[error("invalid PSID/RSID header size {0:#x}")]
    BadHeaderSize(usize),
    /// The cRSID engine could not be initialised.
    #[error("the cRSID engine could not be initialised")]
    EngineInit,
    /// The cRSID engine rejected the tune bytes.
    #[error("the cRSID engine rejected the tune data")]
    Load,
}

/// Something went wrong reading an HVSC Songlengths database (#192).
#[derive(Debug, Error)]
pub enum SongLengthsError {
    /// The database file could not be read.
    #[error("could not read the Songlengths database {path}: {source}")]
    Io {
        /// The database path that failed.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}
