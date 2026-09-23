//! SID (`.sid` / `.psid` / `.rsid`) playback.
//!
//! [`SidDecoder`] is the engine boundary; [`CrsidDecoder`] is the default
//! implementation over the vendored cRSID engine (`emusic-sid`), and
//! [`SidChannel`] plays a tune by feeding a BASS push stream from a background
//! thread. Keeping the engine behind the trait means an alternative engine
//! (e.g. a libsidplayfp helper process, see #61) can replace it later without
//! touching the player/transport code.

mod channel;
mod decoder;

pub use channel::{DEFAULT_TUNE_LENGTH, SID_SAMPLE_RATE, SidChannel};
pub use decoder::{CrsidDecoder, SidDecoder};
