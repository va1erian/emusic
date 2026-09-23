//! The only module in this crate containing `unsafe`: the C ABI of the
//! vendored cRSID engine (compiled from `src/shim.c` by `build.rs`).
//!
//! cRSID is not thread-safe. It keeps process-global state, including the
//! single `cRSID_C64` instance and several function-local `static`s, so two
//! tunes cannot be emulated at once. [`Engine`] therefore holds a process-wide
//! lease for its whole lifetime: constructing a second engine blocks until the
//! first is dropped. The player crate relies on that (it only ever has one
//! tune open) and constructs the engine on the feeder thread that will use it.

use std::ffi::c_void;
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Serialises access to cRSID's process-global state; held for the lifetime of
/// an [`Engine`], so at most one engine exists at a time.
static ENGINE_LEASE: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn emusic_crsid_init(samplerate: u16) -> *mut c_void;
    fn emusic_crsid_process(c64: *mut c_void, data: *mut u8, size: i32) -> *mut c_void;
    fn emusic_crsid_init_tune(c64: *mut c_void, header: *mut c_void, subtune: i32);
    fn emusic_crsid_render(c64: *mut c_void, out: *mut i16, count: i32);
    fn emusic_crsid_override(c64: *mut c_void, chip_model: i32, clock: i32);
}

/// A live cRSID instance plus the lease guaranteeing it is the only one.
pub struct Engine {
    /// The `cRSID_C64instance*` from the C side; never null once constructed.
    c64: *mut c_void,
    /// The `cRSID_SIDheader*` inside the caller's tune buffer; set by
    /// [`Engine::process`].
    header: *mut c_void,
    /// Released when this engine is dropped, letting the next one start.
    _lease: MutexGuard<'static, ()>,
}

impl Engine {
    /// Initialises cRSID at `sample_rate` Hz, waiting for exclusive access.
    ///
    /// Returns `None` if the engine reports a failure.
    pub fn new(sample_rate: u32) -> Option<Self> {
        let lease = ENGINE_LEASE.lock().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: `emusic_crsid_init` wraps `cRSID_init`, which only reads its
        // integer argument and returns a pointer to a process-global struct
        // (or null). No pointer is dereferenced on the Rust side here.
        let c64 = unsafe { emusic_crsid_init(sample_rate as u16) };
        if c64.is_null() {
            return None;
        }
        Some(Self {
            c64,
            header: ptr::null_mut(),
            _lease: lease,
        })
    }

    /// Copies `data` into C64 memory and records the SID header inside it.
    ///
    /// `data` must stay alive and at a stable address until this `Engine` is
    /// dropped: cRSID keeps a pointer into it. Returns `None` if the tune is
    /// rejected.
    pub fn process(&mut self, data: &mut [u8]) -> Option<()> {
        // SAFETY: `self.c64` is the live instance from `new`; `data` is a
        // valid, writable slice of `data.len()` bytes. cRSID reads from it and
        // stores a pointer into it, which the caller keeps alive.
        let header =
            unsafe { emusic_crsid_process(self.c64, data.as_mut_ptr(), data.len() as i32) };
        if header.is_null() {
            return None;
        }
        self.header = header;
        Some(())
    }

    /// Applies chip-model / clock overrides by rewriting the header fields
    /// `cRSID_setC64` reads. Must be called after [`Engine::process`] and
    /// before [`Engine::init_tune`]. `None` leaves the file's own value.
    pub fn override_model(&mut self, chip_model: Option<i32>, clock: Option<i32>) {
        // SAFETY: `self.c64` is live and `self.header` points into the buffer
        // passed to `process`, which the caller keeps alive. The shim only
        // touches the header's model/clock bits.
        unsafe { emusic_crsid_override(self.c64, chip_model.unwrap_or(-1), clock.unwrap_or(-1)) };
    }

    /// Initialises (or restarts) `subtune` (1-based) so the next renders begin
    /// at the start of that subtune.
    pub fn init_tune(&mut self, subtune: u8) {
        // SAFETY: `self.c64` is live and `self.header` was set by a successful
        // `process`. `subtune` is a plain value cRSID clamps itself.
        unsafe { emusic_crsid_init_tune(self.c64, self.header, i32::from(subtune)) };
    }

    /// Fills `out` with mono signed 16-bit samples.
    pub fn render(&mut self, out: &mut [i16]) {
        // SAFETY: `self.c64` is live; `out` is a valid, writable slice of
        // `out.len()` `i16`s, which the shim writes in full.
        unsafe { emusic_crsid_render(self.c64, out.as_mut_ptr(), out.len() as i32) };
    }
}
