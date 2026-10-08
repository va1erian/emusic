//! Text normalization for search values: lowercase + diacritic folding.

/// Lowercases and folds diacritics so that e.g. `Café` matches `cafe`.
///
/// Uses `deunicode` to fold accented and non-Latin characters to their closest
/// ASCII representations before lowercasing.
#[must_use]
pub fn normalize_text(input: &str) -> String {
    // Optimization: For pure ASCII inputs (the vast majority of track metadata and search
    // terms), bypass `deunicode` transliteration and convert directly to ASCII lowercase.
    // Additionally, if the input is already purely lowercase ASCII, return `input.to_string()`
    // directly without running character case conversion loops.
    if input.is_ascii() {
        if !input.as_bytes().iter().any(u8::is_ascii_uppercase) {
            return input.to_string();
        }
        return input.to_ascii_lowercase();
    }

    // `deunicode` output is guaranteed to contain only ASCII characters.
    // Instead of calling `.to_lowercase()` which allocates a second `String`,
    // lowercasing in-place with `.make_ascii_lowercase()` saves 1 heap allocation.
    let mut s = deunicode::deunicode(input);
    s.make_ascii_lowercase();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_ascii() {
        assert_eq!(normalize_text("Hello World"), "hello world");
    }

    #[test]
    fn lowercases_already_lowercase_ascii() {
        assert_eq!(normalize_text("hello world"), "hello world");
    }

    #[test]
    fn folds_diacritics() {
        assert_eq!(normalize_text("Café"), "cafe");
        assert_eq!(normalize_text("ÄÖÜ"), "aou");
    }

    #[test]
    fn folds_sharp_s() {
        assert_eq!(normalize_text("Straße"), "strasse");
    }

    #[test]
    fn empty_string_stays_empty() {
        assert_eq!(normalize_text(""), "");
    }

    #[test]
    fn non_latin_is_transliterated() {
        assert_eq!(normalize_text("Clémentine"), "clementine");
    }
}
