#![forbid(unsafe_code)]

//! Search query parsing for emusic.
//!
//! Turns user input such as `artist:"daft punk" year:1990..1999 -live` into a
//! structured [`Query`](query::Query). Parsing never fails: anything that
//! cannot be interpreted as a field filter is kept as plain text.

pub mod query;

pub use query::{
    Comparison, Field, FieldValue, NumericSpec, Query, Term, TermBody, TextMatch, normalize_text,
    parse,
};
