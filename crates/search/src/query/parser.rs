//! Builds a [`Query`] from raw input by tokenizing and classifying tokens.

use super::ast::{Comparison, Field, FieldValue, NumericSpec, Query, Term, TermBody, TextMatch};
use super::{lexer, normalize};

/// Parses a search query. Never fails and never errors on user input:
/// unknown field prefixes and unparseable numeric values degrade to plain
/// text terms, and tokens with no content are dropped.
#[must_use]
pub fn parse(input: &str) -> Query {
    let terms = lexer::tokenize(input)
        .iter()
        .filter_map(term_from_token)
        .collect();
    Query { terms }
}

/// Converts one raw token into a [`Term`], or drops it if it is empty.
fn term_from_token(token: &lexer::RawToken) -> Option<Term> {
    if token.value.is_empty() {
        return None;
    }
    let body = match &token.field {
        None => TermBody::Text(text_match(&token.value, token.quoted)),
        Some(name) => field_body(name, &token.value, token.quoted),
    };
    Some(Term {
        negated: token.negated,
        body,
    })
}

/// Classifies a token with a `name:` prefix.
fn field_body(name: &str, value: &str, quoted: bool) -> TermBody {
    match Field::parse(name) {
        Some(field) if field.is_numeric() => match parse_numeric(value, field == Field::Duration) {
            Some(spec) => TermBody::Field {
                field,
                value: FieldValue::Numeric(spec),
            },
            // A known numeric field with an unparseable value falls back to text.
            None => TermBody::Text(raw_text_match(name, value, quoted)),
        },
        Some(field) => TermBody::Field {
            field,
            value: FieldValue::Text(text_match(value, quoted)),
        },
        // Unknown field prefixes are treated as plain text.
        None => TermBody::Text(raw_text_match(name, value, quoted)),
    }
}

/// Normalizes a bare value.
fn text_match(value: &str, quoted: bool) -> TextMatch {
    let normalized = normalize::normalize_text(value);
    if quoted {
        TextMatch::Phrase(normalized)
    } else {
        TextMatch::Word(normalized)
    }
}

/// Normalizes a `name:value` token that is being kept as plain text.
fn raw_text_match(name: &str, value: &str, quoted: bool) -> TextMatch {
    text_match(&format!("{name}:{value}"), quoted)
}

/// Parses a numeric filter value: a comparison, a bare value, or a range.
///
/// `duration_units` allows duration suffixes (`s`, `m`, `h`, `sec`, `min`,
/// `hr`) which scale the value to seconds.
fn parse_numeric(value: &str, duration_units: bool) -> Option<NumericSpec> {
    if let Some((lo, hi)) = value.split_once("..") {
        let min = parse_range_bound(lo, duration_units)?;
        let max = parse_range_bound(hi, duration_units)?;
        if min.is_none() && max.is_none() {
            return None;
        }
        return Some(NumericSpec::Range { min, max });
    }

    let (op, rest) = if let Some(rest) = value.strip_prefix(">=") {
        (Comparison::GreaterEqual, rest)
    } else if let Some(rest) = value.strip_prefix("<=") {
        (Comparison::LessEqual, rest)
    } else if let Some(rest) = value.strip_prefix("==") {
        (Comparison::Equal, rest)
    } else if let Some(rest) = value.strip_prefix('>') {
        (Comparison::Greater, rest)
    } else if let Some(rest) = value.strip_prefix('<') {
        (Comparison::Less, rest)
    } else if let Some(rest) = value.strip_prefix('=') {
        (Comparison::Equal, rest)
    } else {
        (Comparison::Equal, value)
    };

    Some(NumericSpec::Compare {
        op,
        value: parse_number(rest, duration_units)?,
    })
}

/// Parses one end of a range; an empty string means "no bound".
fn parse_range_bound(text: &str, duration_units: bool) -> Option<Option<f64>> {
    if text.trim().is_empty() {
        return Some(None);
    }
    Some(Some(parse_number(text, duration_units)?))
}

/// Parses a number, optionally scaled by a duration suffix to seconds.
fn parse_number(text: &str, duration_units: bool) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if duration_units {
        const UNITS: &[(&str, f64)] = &[
            ("sec", 1.0),
            ("s", 1.0),
            ("min", 60.0),
            ("m", 60.0),
            ("hr", 3600.0),
            ("h", 3600.0),
        ];
        for (suffix, scale) in UNITS {
            if let Some(digits) = text.strip_suffix(suffix)
                && let Ok(n) = digits.parse::<f64>()
            {
                return Some(n * scale);
            }
        }
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests;
