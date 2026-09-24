//! Folder view state and the collapsible directory tree (#18).
//!
//! The selection/filter state lives in `emusic-ui` (shared with future
//! frontends); only the egui rendering ([`tree`]) stays here.

mod tree;

pub use tree::show;
