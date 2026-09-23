#![forbid(unsafe_code)]

//! A snapshot of the currently playing SID tune, for the UI.

use super::settings::{SidChipModel, SidClock, SidSettings};

/// Metadata about the loaded SID tune, including the live subtune.
///
/// `chip_model` / `clock` are the *effective* values (the setting override if
/// one is set, otherwise the header's hint).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidInfo {
    /// `PSID` or `RSID`.
    pub format: String,
    /// Tune title (Windows-1252, from the header).
    pub title: String,
    /// Tune author.
    pub author: String,
    /// Release information.
    pub released: String,
    /// Effective chip model, or `None` when the file doesn't say.
    pub chip_model: Option<SidChipModel>,
    /// Effective clock, or `None` when the file doesn't say.
    pub clock: Option<SidClock>,
    /// Total subtunes in the file.
    pub subtune_count: u16,
    /// The header's default subtune.
    pub default_subtune: u16,
    /// The subtune currently playing.
    pub current_subtune: u16,
}

impl SidInfo {
    /// Builds a snapshot from a parsed header, the resolved settings, and the
    /// current subtune.
    pub(crate) fn from_header(
        header: &emusic_sid::SidHeader,
        settings: SidSettings,
        current_subtune: u16,
    ) -> Self {
        let format = match header.format {
            emusic_sid::SidFormat::Psid => "PSID",
            emusic_sid::SidFormat::Rsid => "RSID",
        };
        let chip_model = match settings.chip_model {
            SidChipModel::Auto => header.chip_model.map(chip_model),
            forced => Some(forced),
        };
        let clock = match settings.clock {
            SidClock::Auto => header.clock.map(clock),
            forced => Some(forced),
        };
        Self {
            format: format.to_string(),
            title: header.title.clone(),
            author: header.author.clone(),
            released: header.released.clone(),
            chip_model,
            clock,
            subtune_count: header.subtunes,
            default_subtune: header.default_subtune,
            current_subtune,
        }
    }
}

/// Maps an engine chip model to the player's serde type.
fn chip_model(model: emusic_sid::ChipModel) -> SidChipModel {
    match model {
        emusic_sid::ChipModel::Mos6581 => SidChipModel::Mos6581,
        emusic_sid::ChipModel::Mos8580 => SidChipModel::Mos8580,
    }
}

/// Maps an engine clock to the player's serde type.
fn clock(clock: emusic_sid::Clock) -> SidClock {
    match clock {
        emusic_sid::Clock::Pal => SidClock::Pal,
        emusic_sid::Clock::Ntsc => SidClock::Ntsc,
    }
}
