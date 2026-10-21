//! Natural-order string comparison helpers for the library index.

use std::cmp::Ordering;

/// Compares two strings in a human-friendly order.
///
/// Differences from lexical order:
/// - leading "The " (case-insensitive) is ignored,
/// - sequences of digits are compared by numeric value,
/// - letters are compared case-insensitively.
///
/// This is used to sort artists, albums, genres and directory names in the
/// UI so that "Track 2" appears before "Track 10" and "The Beatles" sorts
/// under "B".
pub fn natural_compare(a: &str, b: &str) -> Ordering {
    let a = strip_the(a);
    let b = strip_the(b);

    // Fast path: For pure ASCII inputs (the vast majority of track, artist, album,
    // genre, and directory names), iterate over bytes directly to avoid UTF-8 char
    // decoding overhead and Unicode lowercasing table lookups.
    if a.is_ascii() && b.is_ascii() {
        return natural_compare_ascii(a.as_bytes(), b.as_bytes());
    }

    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    loop {
        match (a_chars.peek().copied(), b_chars.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ac), Some(bc)) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    let a_num = take_number(&mut a_chars);
                    let b_num = take_number(&mut b_chars);
                    match a_num.cmp(&b_num) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                }

                let ac_lower = if ac.is_ascii() {
                    ac.to_ascii_lowercase()
                } else {
                    ac.to_lowercase().next().unwrap_or(ac)
                };
                let bc_lower = if bc.is_ascii() {
                    bc.to_ascii_lowercase()
                } else {
                    bc.to_lowercase().next().unwrap_or(bc)
                };
                match ac_lower.cmp(&bc_lower) {
                    Ordering::Equal => {
                        a_chars.next();
                        b_chars.next();
                        continue;
                    }
                    other => return other,
                }
            }
        }
    }
}

fn natural_compare_ascii(a: &[u8], b: &[u8]) -> Ordering {
    let mut i = 0;
    let mut j = 0;

    while i < a.len() && j < b.len() {
        let ai = a[i];
        let bj = b[j];

        if ai.is_ascii_digit() && bj.is_ascii_digit() {
            let a_num = take_number_ascii(a, &mut i);
            let b_num = take_number_ascii(b, &mut j);
            match a_num.cmp(&b_num) {
                Ordering::Equal => continue,
                other => return other,
            }
        }

        let a_lower = ai.to_ascii_lowercase();
        let b_lower = bj.to_ascii_lowercase();
        match a_lower.cmp(&b_lower) {
            Ordering::Equal => {
                i += 1;
                j += 1;
            }
            other => return other,
        }
    }

    match (i < a.len(), j < b.len()) {
        (false, false) => Ordering::Equal,
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        (true, true) => unreachable!(),
    }
}

#[inline]
fn take_number_ascii(bytes: &[u8], idx: &mut usize) -> u64 {
    let mut value: u64 = 0;
    while *idx < bytes.len() && bytes[*idx].is_ascii_digit() {
        value = value
            .saturating_mul(10)
            .saturating_add((bytes[*idx] - b'0') as u64);
        *idx += 1;
    }
    value
}

fn strip_the(s: &str) -> &str {
    let trimmed = s.trim_start();
    // `get` rather than `[..4]`: byte 4 may land inside a multi-byte
    // character (e.g. "Mísia"), which would panic when slicing.
    match trimmed.get(..4) {
        Some(prefix) if prefix.eq_ignore_ascii_case("the ") && trimmed.len() > 4 => &trimmed[4..],
        _ => trimmed,
    }
}

fn take_number<I>(chars: &mut std::iter::Peekable<I>) -> u64
where
    I: Iterator<Item = char>,
{
    let mut value: u64 = 0;
    while let Some(&c) = chars.peek() {
        if !c.is_ascii_digit() {
            break;
        }
        value = value
            .saturating_mul(10)
            .saturating_add(c.to_digit(10).unwrap_or(0) as u64);
        chars.next();
    }
    value
}

/// Returns a cheap-to-clone key that can be used with `slice::sort_by`.
pub fn natural_key(s: &str) -> NaturalKey<'_> {
    NaturalKey(s)
}

/// Newtype wrapper around a string slice so it can be compared with
/// [`natural_compare`] when used as a sort key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NaturalKey<'a>(&'a str);

impl PartialOrd for NaturalKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NaturalKey<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        natural_compare(self.0, other.0)
    }
}

#[cfg(test)]
mod tests {

    /// Regression: `strip_the` used to slice `[..4]` by bytes, which panics
    /// when byte 4 falls inside a multi-byte character.
    #[test]
    fn handles_multibyte_names_shorter_than_the_prefix() {
        for name in ["Mísia", "Émilie", "夜の街", "Ü", "Señor", "Ré", "らき"] {
            let _ = natural_compare(name, "The Beatles");
            let _ = natural_compare("The Beatles", name);
            let _ = natural_compare(name, name);
        }
    }

    #[test]
    fn strips_the_only_on_ascii_prefix() {
        assert_eq!(natural_compare("The Beatles", "Beatles"), Ordering::Equal);
        // "Théâtre" must not be treated as "The " + "âtre".
        assert_eq!(natural_compare("Théâtre", "âtre"), Ordering::Less);
    }
    use super::*;

    #[test]
    fn sorts_numbers_numerically() {
        let mut v = vec!["Track 10", "Track 2", "Track 1"];
        v.sort_by(|a, b| natural_compare(a, b));
        assert_eq!(v, vec!["Track 1", "Track 2", "Track 10"]);
    }

    #[test]
    fn ignores_leading_the() {
        let mut v = vec!["The Beatles", "Abba", "The Cure"];
        v.sort_by(|a, b| natural_compare(a, b));
        assert_eq!(v, vec!["Abba", "The Beatles", "The Cure"]);
    }

    #[test]
    fn case_insensitive() {
        let mut v = vec!["apple", "Banana", "apricot"];
        v.sort_by(|a, b| natural_compare(a, b));
        assert_eq!(v, vec!["apple", "apricot", "Banana"]);
    }

    #[test]
    fn leading_zeroes_compare_equal() {
        assert_eq!(natural_compare("Mix 07", "Mix 7"), Ordering::Equal);
    }

    #[test]
    fn ascii_fast_path_edge_cases() {
        assert_eq!(natural_compare("", ""), Ordering::Equal);
        assert_eq!(natural_compare("", "a"), Ordering::Less);
        assert_eq!(natural_compare("a", ""), Ordering::Greater);
        assert_eq!(natural_compare("Track 1", "Track 01"), Ordering::Equal);
        assert_eq!(natural_compare("Track 01", "Track 2"), Ordering::Less);
        assert_eq!(natural_compare("Track 2", "Track 002"), Ordering::Equal);
        assert_eq!(natural_compare("Track 2", "Track 2a"), Ordering::Less);
        assert_eq!(natural_compare("Track 2a", "Track 2b"), Ordering::Less);
        assert_eq!(natural_compare("TRACK 10", "track 2"), Ordering::Greater);
    }
}
