//! Map [`TrackerSettings`] to BASS music flags/attributes and apply them
//! live to a music channel.

use bass::{Attribute, Channel, MusicFlags};

use super::settings::{Emulation, EndBehavior, Interpolation, Ramping, Surround, TrackerSettings};

impl TrackerSettings {
    /// Returns the `BASS_MusicLoad`/`BASS_ChannelFlags` bits that correspond
    /// to this setting.
    pub fn music_flags(&self) -> MusicFlags {
        let mut flags = MusicFlags::empty();

        match self.interpolation {
            Interpolation::None => flags |= MusicFlags::NONINTER,
            Interpolation::Linear => {}
            Interpolation::Sinc => flags |= MusicFlags::SINCINTER,
        }

        match self.ramping {
            Ramping::Off => {}
            Ramping::Normal => flags |= MusicFlags::RAMP,
            Ramping::Sensitive => flags |= MusicFlags::RAMPS,
        }

        match self.surround {
            Surround::Off => {}
            Surround::Mode1 => flags |= MusicFlags::SURROUND,
            Surround::Mode2 => flags |= MusicFlags::SURROUND2,
        }

        if self.emulation == Emulation::Ft2 || self.ft2_pan {
            flags |= MusicFlags::FT2MOD;
        }
        if self.emulation == Emulation::Pt1 {
            flags |= MusicFlags::PT1MOD;
        }

        match self.end {
            EndBehavior::StopAtEnd | EndBehavior::FollowLoops => {}
            EndBehavior::LoopTimes(n) if n > 0 => flags |= MusicFlags::LOOP,
            EndBehavior::LoopTimes(_) => {}
        }

        flags
    }

    /// Mask covering every flag bit this setting can toggle.
    pub fn music_flags_mask() -> u32 {
        MusicFlags::NONINTER.bits()
            | MusicFlags::SINCINTER.bits()
            | MusicFlags::RAMP.bits()
            | MusicFlags::RAMPS.bits()
            | MusicFlags::SURROUND.bits()
            | MusicFlags::SURROUND2.bits()
            | MusicFlags::FT2MOD.bits()
            | MusicFlags::PT1MOD.bits()
            | MusicFlags::LOOP.bits()
            | MusicFlags::STOPBACK.bits()
    }

    /// Applies the channel-specific parts of these settings to `music`:
    /// interpolation, ramping, surround, emulation, looping, amplification
    /// and stereo separation. The global `BASS_CONFIG_SRC` quality is **not**
    /// set here because it requires the [`bass::Bass`] instance; call
    /// [`bass::Config::set_resampling_quality`] separately.
    pub fn apply_to_music(&self, music: &impl Channel) -> Result<(), bass::BassError> {
        let flags = self.music_flags();
        let mask = Self::music_flags_mask();
        music.set_flags(flags.bits(), mask)?;
        music.set_attribute(Attribute::MusicAmplify, f32::from(self.amplify))?;
        music.set_attribute(
            Attribute::MusicPanSeparation,
            f32::from(self.stereo_separation),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::settings::{
        Emulation, EndBehavior, Interpolation, Ramping, Surround, TrackerSettings,
    };

    fn flags_bits(settings: TrackerSettings) -> u32 {
        settings.music_flags().bits()
    }

    #[test]
    fn interpolation_maps_to_flags() {
        assert!(
            TrackerSettings {
                interpolation: Interpolation::None,
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::NONINTER)
        );

        assert!(
            !TrackerSettings {
                interpolation: Interpolation::Linear,
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::NONINTER | MusicFlags::SINCINTER)
        );

        assert!(
            TrackerSettings {
                interpolation: Interpolation::Sinc,
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::SINCINTER)
        );
    }

    #[test]
    fn ramping_maps_to_flags() {
        let off = flags_bits(TrackerSettings {
            ramping: Ramping::Off,
            ..TrackerSettings::default()
        });
        assert_eq!(
            off & (MusicFlags::RAMP.bits() | MusicFlags::RAMPS.bits()),
            0
        );

        let normal = flags_bits(TrackerSettings {
            ramping: Ramping::Normal,
            ..TrackerSettings::default()
        });
        assert_eq!(normal & MusicFlags::RAMP.bits(), MusicFlags::RAMP.bits());
        assert_eq!(normal & MusicFlags::RAMPS.bits(), 0);

        let sensitive = flags_bits(TrackerSettings {
            ramping: Ramping::Sensitive,
            ..TrackerSettings::default()
        });
        assert_eq!(
            sensitive & MusicFlags::RAMPS.bits(),
            MusicFlags::RAMPS.bits()
        );
        assert_eq!(sensitive & MusicFlags::RAMP.bits(), 0);
    }

    #[test]
    fn surround_maps_to_flags() {
        assert!(
            !TrackerSettings {
                surround: Surround::Off,
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::SURROUND | MusicFlags::SURROUND2)
        );

        assert!(
            TrackerSettings {
                surround: Surround::Mode1,
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::SURROUND)
        );

        assert!(
            TrackerSettings {
                surround: Surround::Mode2,
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::SURROUND2)
        );
    }

    #[test]
    fn emulation_and_ft2_pan_share_one_bit() {
        let ft2 = TrackerSettings {
            emulation: Emulation::Ft2,
            ft2_pan: false,
            ..TrackerSettings::default()
        };
        let pan = TrackerSettings {
            emulation: Emulation::Auto,
            ft2_pan: true,
            ..TrackerSettings::default()
        };
        assert_eq!(
            ft2.music_flags().bits() & MusicFlags::FT2MOD.bits(),
            MusicFlags::FT2MOD.bits()
        );
        assert_eq!(
            pan.music_flags().bits() & MusicFlags::FT2MOD.bits(),
            MusicFlags::FT2MOD.bits()
        );

        let pt1 = TrackerSettings {
            emulation: Emulation::Pt1,
            ..TrackerSettings::default()
        };
        assert!(pt1.music_flags().contains(MusicFlags::PT1MOD));
        assert!(!pt1.music_flags().contains(MusicFlags::FT2MOD));
    }

    #[test]
    fn end_behavior_maps_to_loop_flag() {
        assert!(
            !TrackerSettings {
                end: EndBehavior::StopAtEnd,
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::LOOP)
        );

        assert!(
            !TrackerSettings {
                end: EndBehavior::FollowLoops,
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::LOOP)
        );

        assert!(
            TrackerSettings {
                end: EndBehavior::LoopTimes(3),
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::LOOP)
        );

        assert!(
            !TrackerSettings {
                end: EndBehavior::LoopTimes(0),
                ..TrackerSettings::default()
            }
            .music_flags()
            .contains(MusicFlags::LOOP)
        );
    }

    #[test]
    fn mask_includes_all_toggled_bits() {
        let mask = TrackerSettings::music_flags_mask();
        assert!(mask & MusicFlags::NONINTER.bits() != 0);
        assert!(mask & MusicFlags::SINCINTER.bits() != 0);
        assert!(mask & MusicFlags::RAMP.bits() != 0);
        assert!(mask & MusicFlags::RAMPS.bits() != 0);
        assert!(mask & MusicFlags::SURROUND.bits() != 0);
        assert!(mask & MusicFlags::SURROUND2.bits() != 0);
        assert!(mask & MusicFlags::FT2MOD.bits() != 0);
        assert!(mask & MusicFlags::PT1MOD.bits() != 0);
        assert!(mask & MusicFlags::LOOP.bits() != 0);
        assert!(mask & MusicFlags::STOPBACK.bits() != 0);
    }

    #[test]
    fn presets_match_expected_flags() {
        let amiga = TrackerSettings::amiga_authentic();
        assert!(
            amiga
                .music_flags()
                .contains(MusicFlags::NONINTER | MusicFlags::PT1MOD)
        );
        assert!(
            !amiga
                .music_flags()
                .contains(MusicFlags::RAMP | MusicFlags::SINCINTER)
        );

        let smooth = TrackerSettings::smooth();
        assert!(
            smooth
                .music_flags()
                .contains(MusicFlags::SINCINTER | MusicFlags::RAMPS)
        );
        assert_eq!(smooth.stereo_separation, 50);

        let default = TrackerSettings::default();
        assert!(!default.music_flags().intersects(
            MusicFlags::NONINTER
                | MusicFlags::SINCINTER
                | MusicFlags::RAMP
                | MusicFlags::RAMPS
                | MusicFlags::SURROUND
                | MusicFlags::SURROUND2
                | MusicFlags::FT2MOD
                | MusicFlags::PT1MOD
                | MusicFlags::LOOP
        ));
    }
}
