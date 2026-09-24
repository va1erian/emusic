//! Isolated `unsafe` boundary: raw BASS types, constants and the dynamic
//! loader. Every other module in this crate builds a safe API on top of
//! [`BassLib`] and never touches raw pointers or calling conventions
//! itself.
//!
//! This module is `pub` only so [`BassLib`] is visible enough to appear in
//! the [`crate::Channel`] trait's (deliberately `#[doc(hidden)]`) `lib`
//! method — that's what makes `Channel` unimplementable outside this
//! crate, since nothing here can be named or constructed from outside it.
//! It is not meant to be used directly; hence `#[doc(hidden)]` throughout.

#![doc(hidden)]

pub(crate) mod consts;
pub(crate) mod loader;
pub(crate) mod raw;
pub(crate) mod types;

use std::path::Path;
use std::sync::Arc;

use libloading::Library;

use crate::error::BassError;
use raw::RawBindings;

/// The loaded `bass.dll` plus its resolved function table.
///
/// Kept alive for as long as any [`crate::Bass`], [`crate::Stream`] or
/// [`crate::Music`] handle exists, since `raw`'s function pointers point
/// into the mapped library.
pub struct BassLib {
    /// Kept only to keep the DLL mapped; never read directly.
    _library: Library,
    pub(crate) raw: RawBindings,
}

// SAFETY: BASS's own documentation states the library is thread-safe (its
// functions may be called from any thread once initialized). The
// `libloading::Library` handle is an opaque module base address that
// remains valid and immutable for the process lifetime once loaded, and the
// function pointers in `raw` are likewise immutable after `load`. Nothing
// in `BassLib` is mutated after construction, so sharing `&BassLib` or
// moving it across threads is sound.
unsafe impl Send for BassLib {}
// SAFETY: see the `Send` impl above — all fields are read-only after
// construction, so concurrent shared access is sound.
unsafe impl Sync for BassLib {}

impl BassLib {
    /// Loads `bass.dll` from [`loader::bass_dir`] and resolves every symbol
    /// this crate uses.
    pub(crate) fn open() -> Result<Self, BassError> {
        let library = loader::load_dll("bass.dll")?;
        let raw = RawBindings::load(&library)?;
        Ok(Self {
            _library: library,
            raw,
        })
    }

    /// The last error `BASS_ErrorGetCode` reported, as a [`BassError`].
    pub(crate) fn last_error(&self) -> BassError {
        // SAFETY: `BASS_ErrorGetCode` takes no arguments and is safe to
        // call at any time per the BASS documentation.
        let code = unsafe { (self.raw.bass_error_get_code)() };
        BassError::from_code(code)
    }

    /// Runs `f`, mapping a `false`/zero BASS return into `self.last_error()`.
    pub(crate) fn check(&self, ok: bool) -> Result<(), BassError> {
        if ok { Ok(()) } else { Err(self.last_error()) }
    }
}

/// `bassmidi.dll` loaded a second time (independently of the
/// `BASS_PluginLoad` registration [`crate::Bass::load_plugins`] does), to
/// reach its own exports — needed for [`crate::midi::Midi::set_channel_font`]
/// to change an already-open MIDI channel's soundfont live.
///
/// Windows refcounts `LoadLibrary`, so loading a DLL that's already mapped
/// (as a `BASS_PluginLoad`-registered plugin) just returns the same base
/// address rather than mapping it twice.
pub struct MidiLib {
    /// Shares `bass.dll`'s error state: BASS's last-error code is
    /// process-global regardless of which loaded DLL set it.
    bass: Arc<BassLib>,
    /// Kept only to keep the DLL mapped; never read directly.
    _library: Library,
    pub(crate) raw: raw::MidiRawBindings,
}

// SAFETY: see the `Send`/`Sync` impls on `BassLib` above — the same
// reasoning applies: BASS is documented thread-safe and nothing here is
// mutated after construction.
unsafe impl Send for MidiLib {}
// SAFETY: see above.
unsafe impl Sync for MidiLib {}

impl MidiLib {
    /// Loads `bassmidi.dll` from `dir` and resolves the symbols
    /// [`crate::midi`] needs.
    pub(crate) fn open(bass: Arc<BassLib>, dir: &Path) -> Result<Self, BassError> {
        let library = loader::load_dll_from(dir, "bassmidi.dll")?;
        let raw = raw::MidiRawBindings::load(&library)?;
        Ok(Self {
            bass,
            _library: library,
            raw,
        })
    }

    pub(crate) fn last_error(&self) -> BassError {
        self.bass.last_error()
    }
}
