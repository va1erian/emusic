//! SID (`.sid` / `.psid` / `.rsid`) playback.
//!
//! [`SidDecoder`] is the engine boundary; [`CrsidDecoder`] is the default
//! implementation over the vendored cRSID engine (`emusic-sid`), and
//! [`SidChannel`] plays a tune by feeding a BASS push stream from a background
//! thread. Keeping the engine behind the trait means an alternative engine
//! (e.g. a libsidplayfp helper process, see #61) can replace it later without
//! touching the player/transport code.

use std::time::Duration;

#[cfg(feature = "sid")]
mod channel;
#[cfg(feature = "sid")]
mod decoder;

#[cfg(feature = "sid")]
pub use channel::{SID_SAMPLE_RATE, SidChannel};
#[cfg(feature = "sid")]
pub use decoder::{CrsidDecoder, SidDecoder};
pub use emusic_sid::{SongLengths, resolve_database_path};

/// Fallback play length for a SID tune with no HVSC Songlengths entry (#192):
/// long enough to hear the tune, short enough that playback doesn't sit on an
/// unknown-length track forever. The real per-subtune length is used whenever
/// the database has one.
pub const DEFAULT_TUNE_LENGTH: Duration = Duration::from_secs(180);
