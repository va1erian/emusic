//! Shared UI state types (#94): appearance ([`Accent`], [`Theme`],
//! [`Palette`]), [`Command`] messages, [`View`] routing, panel visibility,
//! search popup, settings tabs and visualizer mode.
//!
//! The aggregate `AppState` shell state stays in the egui frontend until
//! the view states move over (#98–#104); everything here is
//! toolkit-agnostic.

mod appearance;
mod command;
mod palette;
mod panels;
mod search;
mod settings;
mod view;
mod visualizer;

pub use appearance::{Accent, DEFAULT_ACCENT, Rgb, Theme};
pub use command::Command;
pub use palette::{Palette, Rgba};
pub use panels::{PanelKind, PanelVisibility};
pub use search::{SearchPopupItem, SearchPopupState};
pub use settings::SettingsTab;
pub use view::View;
pub use visualizer::VisualizerMode;
