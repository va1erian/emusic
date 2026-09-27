//! Tracker module music (`BASS_MusicLoad`): MOD/XM/S3M/IT and friends.

use std::path::Path;
use std::sync::Arc;

use crate::channel::Channel;
use crate::error::BassError;
use crate::ffi::BassLib;
use crate::ffi::types::{Dword, HMusic};
use crate::flags::MusicFlags;
use crate::tags::{MusicTags, read_music_tags};
use crate::util::BassPath;

/// A loaded tracker module, created with `BASS_MusicLoad`.
///
/// Freed automatically (`BASS_MusicFree`) when dropped.
pub struct Music {
    lib: Arc<BassLib>,
    handle: HMusic,
}

impl Music {
    /// Loads `path` as a new tracker module.
    ///
    /// `freq` is the mixing sample rate; `0` uses BASS's default (the
    /// device's output rate).
    pub fn from_file(
        lib: Arc<BassLib>,
        path: impl AsRef<Path>,
        flags: MusicFlags,
        freq: u32,
    ) -> Result<Self, BassError> {
        let path = BassPath::new(path.as_ref())?;
        let raw_flags = flags.bits() | path.flag();
        // SAFETY: `path` is a live, NUL-terminated buffer for the duration of
        // this call; `mem = FALSE` matches the filename pointer we're passing,
        // and `length` is ignored by BASS for file-based loads.
        let handle = unsafe { (lib.raw.bass_music_load)(0, path.as_ptr(), 0, 0, raw_flags, freq) };
        if handle == 0 {
            return Err(lib.last_error());
        }
        Ok(Self { lib, handle })
    }

    /// Reads the module's name/message/instrument/sample tags.
    pub fn tags(&self) -> MusicTags {
        read_music_tags(&self.lib, self.handle)
    }
}

impl Channel for Music {
    fn handle(&self) -> Dword {
        self.handle
    }

    fn lib(&self) -> &Arc<BassLib> {
        &self.lib
    }
}

impl Drop for Music {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was returned by a successful
        // `BASS_MusicLoad` call and hasn't been freed yet (nothing else
        // can free it: `Music` owns it exclusively).
        unsafe { (self.lib.raw.bass_music_free)(self.handle) };
    }
}
