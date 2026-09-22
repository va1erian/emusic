//! Test helper: initialize BASS on its "no sound" device so tests do the
//! same mixing/processing as a real device without making any noise on the
//! developer's speakers.

use std::sync::{Mutex, MutexGuard};

use bass::{Bass, BassError};

/// Serializes tests that need the single live [`Bass`]: BASS's
/// `BASS_Init`/`BASS_Free` state is process-global and the crate allows at
/// most one live instance at a time.
static SERIALIZE: Mutex<()> = Mutex::new(());

/// Acquires the serialization lock and initializes BASS on device `0`, the
/// "no sound" device, at 44100 Hz.
///
/// Returns `None` (after printing why) when `bass.dll` is absent — the
/// expected case on machines without a BASS install — so callers can skip
/// themselves instead of failing. Other initialization errors panic, since
/// they signal a real problem rather than a missing DLL.
pub fn init_silent() -> Option<(MutexGuard<'static, ()>, Bass)> {
    // If a test panicked while holding the lock, recover rather than
    // poisoning every remaining test.
    let guard = SERIALIZE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match Bass::init(0, 44100) {
        Ok(bass) => Some((guard, bass)),
        Err(BassError::DllNotFound(detail)) => {
            eprintln!("skipping: bass.dll not available ({detail})");
            None
        }
        Err(other) => panic!("unexpected error initializing BASS: {other}"),
    }
}
