//! Sample streams (`BASS_StreamCreateFile`): plain audio files (MP3, FLAC,
//! WAV, Opus via plugin, ...).

use std::ffi::c_void;
use std::path::Path;
use std::sync::Arc;

use crate::channel::Channel;
use crate::error::BassError;
use crate::ffi::types::{Dword, HStream};
use crate::ffi::{BassLib, consts as c};
use crate::flags::StreamFlags;
use crate::util::path_to_utf16;

/// A playable audio file stream, created with `BASS_StreamCreateFile`.
///
/// Freed automatically (`BASS_StreamFree`) when dropped.
pub struct Stream {
    lib: Arc<BassLib>,
    handle: HStream,
}

impl Stream {
    /// Opens `path` as a new stream.
    pub fn from_file(
        lib: Arc<BassLib>,
        path: impl AsRef<Path>,
        flags: StreamFlags,
    ) -> Result<Self, BassError> {
        let wide = path_to_utf16(path.as_ref())?;
        let raw_flags = flags.bits() | c::BASS_UNICODE;
        // SAFETY: `wide` is a live, NUL-terminated UTF-16 buffer for the
        // duration of this call; `mem = FALSE` tells BASS to treat the
        // pointer as a filename rather than an in-memory buffer, matching
        // what we're passing.
        let handle = unsafe {
            (lib.raw.bass_stream_create_file)(0, wide.as_ptr() as *const c_void, 0, 0, raw_flags)
        };
        if handle == 0 {
            return Err(lib.last_error());
        }
        Ok(Self { lib, handle })
    }
}

impl Channel for Stream {
    fn handle(&self) -> Dword {
        self.handle
    }

    fn lib(&self) -> &Arc<BassLib> {
        &self.lib
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was returned by a successful
        // `BASS_StreamCreateFile` call and hasn't been freed yet (nothing
        // else can free it: `Stream` owns it exclusively).
        unsafe { (self.lib.raw.bass_stream_free)(self.handle) };
    }
}
