#![forbid(unsafe_code)]

//! Safe, engine-backed player for a single SID tune.
//!
//! [`SidPlayer`] owns the (single) cRSID instance through [`Engine`]; load a
//! tune from bytes, pick a subtune, then repeatedly call [`SidPlayer::render`]
//! to get mono signed 16-bit PCM at the configured sample rate.

use crate::error::SidError;
use crate::ffi::Engine;
use crate::header::{ChipModel, Clock, SidHeader};

/// Lowest sample rate the engine is asked to run at.
pub const MIN_SAMPLE_RATE: u32 = 8_000;
/// Highest sample rate the engine is asked to run at.
pub const MAX_SAMPLE_RATE: u32 = 48_000;

/// Optional overrides for a tune, applied on top of its header.
///
/// `None` fields keep whatever the file asks for (which the engine defaults to
/// 6581/PAL when the header is silent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SidConfig {
    /// Force the SID chip model.
    pub chip_model: Option<ChipModel>,
    /// Force the video standard / clock.
    pub clock: Option<Clock>,
    /// Start at this subtune instead of the header's default.
    pub subtune: Option<u16>,
}

/// A loaded SID tune that renders PCM.
///
/// Not `Send`: cRSID's emulation state is process-global and the crate's
/// [`Engine`] lease is held for this value's lifetime. Create and use it on one
/// thread (the player crate creates it on its feeder thread).
pub struct SidPlayer {
    engine: Engine,
    /// The tune bytes. cRSID keeps a header pointer into this buffer, so it
    /// must outlive `engine` and stay put on the heap. `engine` is declared
    /// first so it drops first (Rust drops fields in declaration order). The
    /// bytes are never read back, only owned as the engine's backing store.
    #[allow(dead_code)]
    data: Vec<u8>,
    header: SidHeader,
    sample_rate: u32,
    current_subtune: u16,
}

impl SidPlayer {
    /// Loads `data` as a SID tune and prepares `sample_rate` Hz rendering.
    ///
    /// `sample_rate` is clamped to [`MIN_SAMPLE_RATE`]`..=`[`MAX_SAMPLE_RATE`].
    pub fn from_bytes(
        data: Vec<u8>,
        sample_rate: u32,
        config: SidConfig,
    ) -> Result<Self, SidError> {
        let header = SidHeader::parse(&data)?;
        let sample_rate = sample_rate.clamp(MIN_SAMPLE_RATE, MAX_SAMPLE_RATE);

        let mut engine = Engine::new(sample_rate).ok_or(SidError::EngineInit)?;
        let mut data = data;
        engine.process(&mut data).ok_or(SidError::Load)?;
        engine.override_model(
            config.chip_model.map(chip_model_code),
            config.clock.map(clock_code),
        );

        let requested = config.subtune.unwrap_or(header.default_subtune);
        let subtune = requested.clamp(1, header.subtunes);
        let mut player = Self {
            engine,
            data,
            header,
            sample_rate,
            current_subtune: subtune,
        };
        player.restart()?;
        Ok(player)
    }

    /// The parsed header.
    pub fn header(&self) -> &SidHeader {
        &self.header
    }

    /// The rendering sample rate, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// How many subtunes the tune contains (`1..=256`).
    pub fn subtune_count(&self) -> u16 {
        self.header.subtunes
    }

    /// The header's default subtune (`1..=subtune_count`).
    pub fn default_subtune(&self) -> u16 {
        self.header.default_subtune
    }

    /// The subtune currently selected.
    pub fn current_subtune(&self) -> u16 {
        self.current_subtune
    }

    /// Switches to `subtune` (clamped to the valid range) and restarts it.
    pub fn select_subtune(&mut self, subtune: u16) -> Result<(), SidError> {
        self.current_subtune = subtune.clamp(1, self.header.subtunes);
        self.restart()
    }

    /// Restarts the current subtune from its init routine.
    pub fn restart(&mut self) -> Result<(), SidError> {
        // cRSID's `cRSID_initSIDtune` returns nothing and cannot fail for a
        // tune that `process` already accepted, but the `Result` keeps the API
        // future-proof for an engine that can.
        self.engine.init_tune(self.engine_subtune());
        Ok(())
    }

    /// Renders `out.len()` mono signed 16-bit samples, returning how many were
    /// written (always `out.len()`).
    pub fn render(&mut self, out: &mut [i16]) -> usize {
        self.engine.render(out);
        out.len()
    }

    /// The subtune as the engine's 1-byte field (capped at 255; cRSID itself
    /// cannot represent subtune 256).
    fn engine_subtune(&self) -> u8 {
        self.current_subtune.min(u16::from(u8::MAX)) as u8
    }
}

/// Encodes a [`ChipModel`] the way `src/shim.c` expects.
fn chip_model_code(model: ChipModel) -> i32 {
    match model {
        ChipModel::Mos6581 => 6581,
        ChipModel::Mos8580 => 8580,
    }
}

/// Encodes a [`Clock`] the way `src/shim.c` expects (`0` = NTSC, `1` = PAL).
fn clock_code(clock: Clock) -> i32 {
    match clock {
        Clock::Ntsc => 0,
        Clock::Pal => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_non_sid_file() {
        let data = vec![0u8; 256];
        assert!(matches!(
            SidPlayer::from_bytes(data, 44_100, SidConfig::default()),
            Err(SidError::BadMagic)
        ));
    }

    #[test]
    fn rejects_a_truncated_file() {
        assert!(matches!(
            SidPlayer::from_bytes(vec![0u8; 4], 44_100, SidConfig::default()),
            Err(SidError::TooSmall)
        ));
    }
}
