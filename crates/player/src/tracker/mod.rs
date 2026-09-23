//! Tracker module playback configuration: settings, presets, BASS flag/attribute
//! mapping and resolution order.

pub mod apply;
pub mod info;
pub mod resolution;
pub mod settings;

pub use info::ModuleInfo;
pub use resolution::{TrackerConfig, TrackerFormat};
pub use settings::{Emulation, EndBehavior, Interpolation, Ramping, Surround, TrackerSettings};

#[cfg(test)]
mod tests;
