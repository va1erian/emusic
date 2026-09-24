//! Track-table view model: column definitions and sorting (#15, #93).
//!
//! Rendering (cells, stars, playing marker) stays in the frontends; this
//! module holds the column identities/widths/text and the sort order built
//! from header clicks.

pub mod columns;
pub mod sort;
