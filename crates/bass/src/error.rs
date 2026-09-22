//! Error type returned by every fallible operation in this crate.

use thiserror::Error;

use crate::ffi::consts::*;

/// Everything that can go wrong using BASS, including this wrapper's own
/// dynamic-loading concerns.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum BassError {
    /// Not enough memory.
    #[error("BASS: out of memory")]
    Mem,
    /// The file could not be opened.
    #[error("BASS: can't open the file")]
    FileOpen,
    /// Can't find a free/valid driver.
    #[error("BASS: can't find a free/valid driver")]
    Driver,
    /// The sample buffer was lost.
    #[error("BASS: sample buffer was lost")]
    BufLost,
    /// Invalid handle.
    #[error("BASS: invalid handle")]
    Handle,
    /// Unsupported sample format.
    #[error("BASS: unsupported sample format")]
    Format,
    /// Invalid position.
    #[error("BASS: invalid position")]
    Position,
    /// `BASS_Init` has not been successfully called.
    #[error("BASS: not initialized")]
    Init,
    /// `BASS_Start` has not been successfully called.
    #[error("BASS: not started")]
    Start,
    /// SSL/HTTPS support isn't available.
    #[error("BASS: SSL/HTTPS unavailable")]
    Ssl,
    /// The device needs to be reinitialized.
    #[error("BASS: device needs reinitializing")]
    Reinit,
    /// Already initialized/paused/whatever.
    #[error("BASS: already done")]
    Already,
    /// The file does not contain audio.
    #[error("BASS: file has no audio")]
    NotAudio,
    /// Can't get a free channel.
    #[error("BASS: no free channel")]
    NoChan,
    /// An illegal type was specified.
    #[error("BASS: illegal type")]
    IllType,
    /// An illegal parameter was specified.
    #[error("BASS: illegal parameter")]
    IllParam,
    /// No 3D support.
    #[error("BASS: no 3D support")]
    No3d,
    /// No EAX support.
    #[error("BASS: no EAX support")]
    NoEax,
    /// Illegal device number.
    #[error("BASS: illegal device number")]
    Device,
    /// Not playing.
    #[error("BASS: not playing")]
    NoPlay,
    /// Illegal sample rate.
    #[error("BASS: illegal sample rate")]
    Freq,
    /// The stream is not a file stream.
    #[error("BASS: not a file stream")]
    NotFile,
    /// No hardware voices available.
    #[error("BASS: no hardware voices available")]
    NoHw,
    /// The MOD music has no sequence data.
    #[error("BASS: MOD music has no sequence data")]
    Empty,
    /// No internet connection could be opened.
    #[error("BASS: no internet connection")]
    NoNet,
    /// Couldn't create the file.
    #[error("BASS: couldn't create file")]
    Create,
    /// Effects are not available.
    #[error("BASS: effects unavailable")]
    NoFx,
    /// Requested data/action is not available.
    #[error("BASS: not available")]
    NotAvail,
    /// The channel is/isn't a "decoding channel".
    #[error("BASS: decode-channel mismatch")]
    Decode,
    /// A sufficient DirectX version is not installed.
    #[error("BASS: insufficient DirectX version")]
    Dx,
    /// Connection timed out.
    #[error("BASS: timed out")]
    Timeout,
    /// Unsupported file format.
    #[error("BASS: unsupported file format")]
    FileForm,
    /// Unavailable speaker.
    #[error("BASS: unavailable speaker")]
    Speaker,
    /// Invalid BASS version (used by add-ons).
    #[error("BASS: invalid BASS version")]
    Version,
    /// Codec is not available/supported.
    #[error("BASS: codec unavailable")]
    Codec,
    /// The channel/file has ended.
    #[error("BASS: channel has ended")]
    Ended,
    /// The device is busy.
    #[error("BASS: device busy")]
    Busy,
    /// Unstreamable file.
    #[error("BASS: unstreamable file")]
    Unstreamable,
    /// Unsupported protocol.
    #[error("BASS: unsupported protocol")]
    Protocol,
    /// Access denied.
    #[error("BASS: access denied")]
    Denied,
    /// Any BASS error code not mapped to a specific variant above.
    #[error("BASS: unknown error (code {0})")]
    Unknown(i32),

    /// `bass.dll` (or a plugin) could not be found or loaded.
    #[error("BASS DLL not found or couldn't be loaded: {0}")]
    DllNotFound(String),
    /// A required entry point was missing from `bass.dll`.
    #[error("BASS symbol not found: {0}")]
    SymbolNotFound(String),
    /// A path could not be represented as BASS expects (e.g. contains an
    /// embedded NUL byte).
    #[error("invalid path for BASS: {0}")]
    InvalidPath(String),
}

impl BassError {
    /// Maps a `BASS_ErrorGetCode()` return value to a [`BassError`].
    pub(crate) fn from_code(code: i32) -> Self {
        match code {
            BASS_ERROR_MEM => Self::Mem,
            BASS_ERROR_FILEOPEN => Self::FileOpen,
            BASS_ERROR_DRIVER => Self::Driver,
            BASS_ERROR_BUFLOST => Self::BufLost,
            BASS_ERROR_HANDLE => Self::Handle,
            BASS_ERROR_FORMAT => Self::Format,
            BASS_ERROR_POSITION => Self::Position,
            BASS_ERROR_INIT => Self::Init,
            BASS_ERROR_START => Self::Start,
            BASS_ERROR_SSL => Self::Ssl,
            BASS_ERROR_REINIT => Self::Reinit,
            BASS_ERROR_ALREADY => Self::Already,
            BASS_ERROR_NOTAUDIO => Self::NotAudio,
            BASS_ERROR_NOCHAN => Self::NoChan,
            BASS_ERROR_ILLTYPE => Self::IllType,
            BASS_ERROR_ILLPARAM => Self::IllParam,
            BASS_ERROR_NO3D => Self::No3d,
            BASS_ERROR_NOEAX => Self::NoEax,
            BASS_ERROR_DEVICE => Self::Device,
            BASS_ERROR_NOPLAY => Self::NoPlay,
            BASS_ERROR_FREQ => Self::Freq,
            BASS_ERROR_NOTFILE => Self::NotFile,
            BASS_ERROR_NOHW => Self::NoHw,
            BASS_ERROR_EMPTY => Self::Empty,
            BASS_ERROR_NONET => Self::NoNet,
            BASS_ERROR_CREATE => Self::Create,
            BASS_ERROR_NOFX => Self::NoFx,
            BASS_ERROR_NOTAVAIL => Self::NotAvail,
            BASS_ERROR_DECODE => Self::Decode,
            BASS_ERROR_DX => Self::Dx,
            BASS_ERROR_TIMEOUT => Self::Timeout,
            BASS_ERROR_FILEFORM => Self::FileForm,
            BASS_ERROR_SPEAKER => Self::Speaker,
            BASS_ERROR_VERSION => Self::Version,
            BASS_ERROR_CODEC => Self::Codec,
            BASS_ERROR_ENDED => Self::Ended,
            BASS_ERROR_BUSY => Self::Busy,
            BASS_ERROR_UNSTREAMABLE => Self::Unstreamable,
            BASS_ERROR_PROTOCOL => Self::Protocol,
            BASS_ERROR_DENIED => Self::Denied,
            other => Self::Unknown(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_codes() {
        assert_eq!(BassError::from_code(BASS_ERROR_HANDLE), BassError::Handle);
        assert_eq!(BassError::from_code(BASS_ERROR_ENDED), BassError::Ended);
    }

    #[test]
    fn unknown_code_is_preserved() {
        assert_eq!(BassError::from_code(12345), BassError::Unknown(12345));
    }
}
