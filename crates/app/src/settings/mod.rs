//! Application settings widgets: sub-pages that need more than the
//! single-file placeholder in `views::settings` ([`tracker`] for tracker
//! module playback options, [`associations`] for file associations, #11,
//! [`library`] for library folders, #19, [`folder_picker`] for the
//! non-blocking native folder dialog, #69).

pub mod about;
pub mod associations;
pub use emusic_ui::folder_picker;
pub mod library;
pub mod midi;
pub mod tracker;
