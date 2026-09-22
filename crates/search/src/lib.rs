#![forbid(unsafe_code)]

//! Search query parsing and in-memory matching for emusic.
//!
//! Turns user input such as `artist:"daft punk" year:1990..1999 -live` into a
//! structured [`Query`](query::Query) and matches it against an [`Index`](matcher::Index)
//! of tracks.

pub mod matcher;
pub mod query;

pub use matcher::{Index, PlayStats, search};
pub use query::{
    Comparison, Field, FieldValue, NumericSpec, Query, Term, TermBody, TextMatch, normalize_text,
    parse,
};
