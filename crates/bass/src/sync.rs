//! `BASS_ChannelSetSync` wrapper: safe callbacks for channel events.

use std::ffi::c_void;
use std::sync::Arc;

use crate::error::BassError;
use crate::ffi::BassLib;
use crate::ffi::consts as c;
use crate::ffi::types::{Dword, HSync};

/// A boxed, type-erased callback registered with BASS.
type Callback = Box<dyn Fn() + Send + 'static>;

/// A registered channel sync (event callback).
///
/// Removes itself from BASS and frees the callback when dropped, so keep
/// this alive for as long as you want the callback to fire.
pub struct ChannelSync {
    lib: Arc<BassLib>,
    channel: Dword,
    handle: HSync,
    /// Owning pointer to the boxed callback BASS holds a raw reference to;
    /// reclaimed and dropped in [`Drop::drop`].
    callback: *mut Callback,
}

// SAFETY: `ChannelSync` only exposes drop semantics; the `Callback` itself
// is required to be `Send` at construction (see `register`), and the raw
// pointer is never dereferenced except by BASS's own thread (via the
// trampoline) or by us while removing the sync, never concurrently with
// itself since `BASS_ChannelRemoveSync` happens-before the pointer is
// freed.
unsafe impl Send for ChannelSync {}

impl ChannelSync {
    /// Registers `callback` to run when `channel` reaches the end
    /// (`BASS_SYNC_END`).
    pub(crate) fn register(
        lib: Arc<BassLib>,
        channel: Dword,
        callback: impl Fn() + Send + 'static,
    ) -> Result<Self, BassError> {
        let boxed: Callback = Box::new(callback);
        let ptr = Box::into_raw(Box::new(boxed));

        // SAFETY: `trampoline` matches `SyncProc`'s signature exactly, and
        // `ptr` is a valid `*mut Callback` that stays alive at least until
        // we remove the sync below (we only free it after that).
        let handle = unsafe {
            (lib.raw.bass_channel_set_sync)(
                channel,
                c::BASS_SYNC_END,
                0,
                trampoline,
                ptr as *mut c_void,
            )
        };
        if handle == 0 {
            let err = lib.last_error();
            // SAFETY: registration failed, so BASS never saw `ptr`; we own
            // it exclusively and can drop it immediately.
            drop(unsafe { Box::from_raw(ptr) });
            return Err(err);
        }

        Ok(Self {
            lib,
            channel,
            handle,
            callback: ptr,
        })
    }
}

impl Drop for ChannelSync {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was returned by a successful
        // `BASS_ChannelSetSync` call in `register` and hasn't been removed
        // yet.
        unsafe { (self.lib.raw.bass_channel_remove_sync)(self.channel, self.handle) };
        // SAFETY: BASS will not invoke the trampoline for this sync again
        // after `BASS_ChannelRemoveSync` returns, so we're the sole owner
        // of `self.callback` again and can free it.
        drop(unsafe { Box::from_raw(self.callback) });
    }
}

/// # Safety
/// `user` must be a valid, live `*mut Callback` as set up by
/// [`ChannelSync::register`]; BASS must not call this after the
/// corresponding [`ChannelSync`] has started dropping.
unsafe extern "system" fn trampoline(
    _handle: HSync,
    _channel: Dword,
    _data: Dword,
    user: *mut c_void,
) {
    // SAFETY: caller (BASS) guarantees `user` is still the live pointer
    // `register` passed it, per this function's safety contract.
    let callback = unsafe { &*(user as *const Callback) };
    callback();
}
