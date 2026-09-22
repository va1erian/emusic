//! Numeric filter evaluation against track fields and external stats.

use crate::query::{Comparison, Field, NumericSpec};

/// Returns the numeric value of a track field, if it is present.
///
/// [`Field::Plays`] is intentionally excluded: play counts live outside the
/// track and are provided through [`PlayStats`](super::PlayStats).
pub fn track_value(field: Field, track: &emusic_core::Track) -> Option<f64> {
    match field {
        Field::Year => track.year.map(f64::from),
        Field::Duration => Some(f64::from(track.duration_ms) / 1000.0),
        _ => None,
    }
}

/// Checks whether `value` satisfies `spec`.
pub fn matches(spec: NumericSpec, value: f64) -> bool {
    match spec {
        NumericSpec::Compare { op, value: target } => compare(op, value, target),
        NumericSpec::Range { min, max } => {
            min.is_none_or(|lower| value >= lower) && max.is_none_or(|upper| value <= upper)
        }
    }
}

fn compare(op: Comparison, value: f64, target: f64) -> bool {
    match op {
        Comparison::Less => value < target,
        Comparison::LessEqual => value <= target,
        Comparison::Equal => value == target,
        Comparison::GreaterEqual => value >= target,
        Comparison::Greater => value > target,
    }
}

#[cfg(test)]
mod tests;
