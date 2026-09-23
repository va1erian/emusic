//! Live tracker-module playback metadata, read back from the backend
//! channel while a module is playing.

/// Live tracker-module metadata for the currently playing module: name,
/// format, current order/row and instrument/sample names.
///
/// Only produced for tracker formats (MOD/XM/IT/S3M and friends); a plain
/// audio stream has none of this.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleInfo {
    /// The module's embedded title, if it has one.
    pub name: String,
    /// Human-readable format name (e.g. `"IT"`, `"XM"`, `"MOD"`).
    pub format: String,
    /// Number of tracker channels the module uses.
    pub channels: u32,
    /// Total number of orders (pattern sequence entries) in the module.
    pub orders: u32,
    /// The order currently playing.
    pub current_order: u32,
    /// The row within the current order currently playing.
    pub current_row: u32,
    /// The module's embedded message/comment text, if any.
    pub message: String,
    /// Instrument names, in order (index = instrument number).
    pub instruments: Vec<String>,
    /// Sample names, in order (index = sample number).
    pub samples: Vec<String>,
}
