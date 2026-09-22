//! Query parsing: AST types, lexer, parser and text normalization.

mod ast;
mod lexer;
mod parser;

pub mod normalize;

pub use ast::{Comparison, Field, FieldValue, NumericSpec, Query, Term, TermBody, TextMatch};
pub use normalize::normalize_text;
pub use parser::parse;
