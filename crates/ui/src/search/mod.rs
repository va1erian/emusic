//! Search plumbing that wires `emusic-search`'s parser and matcher into the
//! shell.
//!
//! [`SearchEngine`] owns a small background worker: it keeps a precomputed
//! `emusic_search::Index` built from the library's tracks and answers query
//! strings against it without blocking the UI thread. Rebuilding the index
//! (which folds every track's text fields once) only happens when the track
//! set actually changes; typing a new character just re-runs the (already
//! prepared) query against the existing haystacks.

mod engine;
mod index;

pub use engine::SearchEngine;

/// Query syntax help shown in the search box's tooltip.
pub const QUERY_HELP: &str = "Type to filter live. Examples:\n\
     artist:daft punk\n\
     year:1990..1999\n\
     plays:>10\n\
     -live  (exclude a word)\n\
     \"exact phrase\"\n\
     Fields: artist, album, albumartist, title, genre, file, dir, ext, year, plays, duration";
