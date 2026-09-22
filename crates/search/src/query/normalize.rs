//! Text normalization for search values: lowercase + diacritic folding.

/// Lowercases and folds diacritics so that e.g. `Café` matches `cafe`.
///
/// Uses `deunicode` to fold accented and non-Latin characters to their closest
/// ASCII representations before lowercasing.
#[must_use]
pub fn normalize_text(input: &str) -> String {
    deunicode::deunicode(input).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_ascii() {
        assert_eq!(normalize_text("Hello World"), "hello world");
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
