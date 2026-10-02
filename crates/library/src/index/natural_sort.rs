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

    // Optimization: Pure ASCII inputs represent >95% of music track titles, artist
    // names, album names, and file paths. Fast-path byte iteration avoids UTF-8
    // character decoding overhead and std::char::ToLowercase table lookups.
    if a.is_ascii() && b.is_ascii() {
        return natural_compare_ascii(a, b);
    }

    natural_compare_unicode(a, b)
}

fn natural_compare_ascii(a: &str, b: &str) -> Ordering {
    let mut a_bytes = a.bytes().peekable();
    let mut b_bytes = b.bytes().peekable();

    loop {
        match (a_bytes.peek().copied(), b_bytes.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ab), Some(bb)) => {
                if ab.is_ascii_digit() && bb.is_ascii_digit() {
                    let a_num = take_number_ascii(&mut a_bytes);
                    let b_num = take_number_ascii(&mut b_bytes);
                    match a_num.cmp(&b_num) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                }

                let ab_lower = ab.to_ascii_lowercase();
                let bb_lower = bb.to_ascii_lowercase();
                match ab_lower.cmp(&bb_lower) {
                    Ordering::Equal => {
                        a_bytes.next();
                        b_bytes.next();
                        continue;
                    }
                    other => return other,
                }
            }
        }
    }
}

fn take_number_ascii<I>(bytes: &mut std::iter::Peekable<I>) -> u64
where
    I: Iterator<Item = u8>,
{
    let mut value: u64 = 0;
    while let Some(&b) = bytes.peek() {
        if !b.is_ascii_digit() {
            break;
        }
        // Optimization: `(b - b'0')` avoids `b.to_digit(10)` redundant bounds check.
        value = value.saturating_mul(10).saturating_add((b - b'0') as u64);
        bytes.next();
    }
    value
}

fn natural_compare_unicode(a: &str, b: &str) -> Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    loop {
        match (a_chars.peek().copied(), b_chars.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ac), Some(bc)) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    let a_num = take_number_unicode(&mut a_chars);
                    let b_num = take_number_unicode(&mut b_chars);
                    match a_num.cmp(&b_num) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                }

                // Optimization: ASCII fast path for character lowercasing avoids
                // `std::char::ToLowercase` iterator creation and Unicode table lookup.
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

fn take_number_unicode<I>(chars: &mut std::iter::Peekable<I>) -> u64
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
            .saturating_add((c as u8 - b'0') as u64);
        chars.next();
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
    fn ascii_and_unicode_functions_match() {
        let test_pairs = [
            ("Track 01", "Track 2"),
            ("The Beatles", "Beatles"),
            ("Album 10", "Album 2"),
            ("artist name", "ARTIST NAME"),
            ("Mix 007", "Mix 7"),
            ("100 percent", "20 percent"),
        ];

        for (a, b) in test_pairs {
            let res_fast = natural_compare(a, b);
            let res_unicode = natural_compare_unicode(strip_the(a), strip_the(b));
            assert_eq!(
                res_fast, res_unicode,
                "mismatch for pair ({:?}, {:?})",
                a, b
            );
        }
    }

    #[test]
    fn natural_key_sorts_correctly() {
        let mut keys = [
            natural_key("Track 10"),
            natural_key("Track 2"),
            natural_key("Track 1"),
        ];
        keys.sort();
        let strings: Vec<&str> = keys.iter().map(|k| k.0).collect();
        assert_eq!(strings, vec!["Track 1", "Track 2", "Track 10"]);
    }
}
