#![forbid(unsafe_code)]

//! Errors returned by the safe SID API.

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
