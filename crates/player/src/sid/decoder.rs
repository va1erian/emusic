#![forbid(unsafe_code)]

//! The engine-agnostic SID decoder seam.
//!
//! [`SidDecoder`] is deliberately narrow: everything the playback/feeder code
//! needs and nothing engine-specific. The default implementation,
//! [`CrsidDecoder`], wraps the vendored cRSID engine (`emusic-sid`). An
//! alternative engine — e.g. a libsidplayfp helper process (#61) — only has to
//! implement this trait; callers never change.

use crate::error::PlayerError;

/// A loaded SID tune that can render PCM and switch subtunes.
///
/// Implementations render mono signed 16-bit samples (the player converts to
/// the push stream's float format). This is intentionally not `Send`: the
/// default engine keeps process-global state, so it is created and used on the
/// single feeder thread that owns it.
pub trait SidDecoder {
    /// Fills `out` with mono signed 16-bit samples, returning how many were
    /// written.
    fn render(&mut self, out: &mut [i16]) -> Result<usize, PlayerError>;

    /// Restarts the current subtune from its init routine.
    fn restart(&mut self) -> Result<(), PlayerError>;

    /// Switches to `subtune` (1-based, clamped to the valid range).
    fn select_subtune(&mut self, subtune: u16) -> Result<(), PlayerError>;

    /// How many subtunes the tune contains.
    fn subtune_count(&self) -> u16;

    /// The header's default subtune.
    fn default_subtune(&self) -> u16;

    /// The subtune currently selected.
    fn current_subtune(&self) -> u16;
}

/// The default [`SidDecoder`], backed by the vendored cRSID engine.
pub struct CrsidDecoder {
    inner: emusic_sid::SidPlayer,
}

impl CrsidDecoder {
    /// Loads a tune from its raw file bytes.
    pub fn from_bytes(
        data: Vec<u8>,
        sample_rate: u32,
        config: emusic_sid::SidConfig,
    ) -> Result<Self, PlayerError> {
        Ok(Self {
            inner: emusic_sid::SidPlayer::from_bytes(data, sample_rate, config)?,
        })
    }

    /// The parsed PSID/RSID header, for metadata display (#65).
    pub fn header(&self) -> &emusic_sid::SidHeader {
        self.inner.header()
    }
}

impl SidDecoder for CrsidDecoder {
    fn render(&mut self, out: &mut [i16]) -> Result<usize, PlayerError> {
        Ok(self.inner.render(out))
    }

    fn restart(&mut self) -> Result<(), PlayerError> {
        self.inner.restart().map_err(PlayerError::Sid)
    }

    fn select_subtune(&mut self, subtune: u16) -> Result<(), PlayerError> {
        self.inner.select_subtune(subtune).map_err(PlayerError::Sid)
    }

    fn subtune_count(&self) -> u16 {
        self.inner.subtune_count()
    }

    fn default_subtune(&self) -> u16 {
        self.inner.default_subtune()
    }

    fn current_subtune(&self) -> u16 {
        self.inner.current_subtune()
    }
}
