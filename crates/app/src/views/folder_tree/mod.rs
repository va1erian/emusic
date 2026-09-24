//! egui rendering of the Folders view's collapsible directory tree (#18,
//! #101). The state and filter live in `emusic-ui` ([`FoldersView`]); only
//! the tree drawing stays here.

mod tree;

pub use tree::show;
