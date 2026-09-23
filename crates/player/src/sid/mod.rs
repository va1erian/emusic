//! SID (`.sid` / `.psid` / `.rsid`) playback.
//!
//! [`SidDecoder`] is the engine boundary; [`CrsidDecoder`] is the default
//! implementation over the vendored cRSID engine (`emusic-sid`), and
//! [`SidChannel`] plays a tune by feeding a BASS push stream from a background
//! thread. Keeping the engine behind the trait means an alternative engine
//! (e.g. a libsidplayfp helper process, see #61) can replace it later without
//! touching the player/transport code.
//!
//! [`SidConfig`] carries chip-model / clock / default-length settings with the
//! same three-tier (global / per-format / per-file) resolution as
//! [`crate::tracker`]. [`HvscIndex`] re-exports the optional HVSC lookups
//! (song lengths and STIL) from `emusic-sid`.

mod channel;
mod decoder;
mod info;
mod settings;

pub use channel::{SID_SAMPLE_RATE, SidChannel};
pub use decoder::{CrsidDecoder, SidDecoder};
pub use info::SidInfo;
pub use settings::{
    DEFAULT_SONG_LENGTH_SECS, SidChipModel, SidClock, SidConfig, SidFormat, SidSettings,
};

pub use emusic_sid::{HvscError, HvscIndex};
