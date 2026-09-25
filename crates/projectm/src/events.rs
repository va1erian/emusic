#![forbid(unsafe_code)]

//! [`Event`]: what the playlist reported while it switched presets.

/// A playlist callback, queued while projectM runs and read back with
/// [`crate::Instance::take_events`] instead of calling into app code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The playlist moved to the preset at `index`.
    PresetSwitched {
        /// Playlist index now shown.
        index: usize,
        /// Whether it cut in immediately rather than blending.
        hard_cut: bool,
    },
    /// A preset failed to load; the playlist moves on to another one.
    PresetFailed {
        /// The preset file.
        file: String,
        /// projectM's reason.
        message: String,
    },
}
