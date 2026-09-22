//! Splits raw query input into tokens for the parser.

/// A raw token before field validation and normalization.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawToken {
    /// Whether the token had a leading `-`.
    pub negated: bool,
    /// Field prefix as written by the user, if the token had a `name:` prefix.
    pub field: Option<String>,
    /// Value text with quotes stripped.
    pub value: String,
    /// Whether the value contained (or was wrapped in) quotes.
    pub quoted: bool,
}

/// Tokenizes `input` on whitespace, honoring quotes and `-`/`name:` prefixes.
///
/// Never errors: malformed input (stray quotes, dangling prefixes) yields the
/// closest sensible token, and tokens with no content are dropped.
pub(crate) fn tokenize(input: &str) -> Vec<RawToken> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let (token, next) = scan_token(&chars, i);
        if let Some(token) = token {
            tokens.push(token);
        }
        i = next;
    }
    tokens
}

/// Scans one token starting at `start`, returning it and the index after it.
fn scan_token(chars: &[char], start: usize) -> (Option<RawToken>, usize) {
    let mut i = start;
    let mut negated = false;
    if chars[i] == '-' && chars.get(i + 1).is_some_and(|&c| !c.is_whitespace()) {
        negated = true;
        i += 1;
    }

    let mut field = None;
    if let Some((name, after)) = scan_field_prefix(chars, i) {
        field = Some(name);
        i = after;
    }

    let mut value = String::new();
    let mut quoted = false;
    while i < chars.len() {
        match chars[i] {
            c if c.is_whitespace() => break,
            '"' => {
                quoted = true;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    value.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    i += 1; // skip the closing quote; an unclosed quote just ends the token
                }
            }
            c => {
                value.push(c);
                i += 1;
            }
        }
    }

    if value.is_empty() {
        // Nothing usable (e.g. `-`, `""`, or `artist:` with no value): drop it.
        (None, i)
    } else {
        (
            Some(RawToken {
                negated,
                field,
                value,
                quoted,
            }),
            i,
        )
    }
}

/// If an identifier followed by `:` starts at `start`, returns its name and the
/// index just after the colon. Unknown names are returned too; the parser
/// decides what to do with them.
fn scan_field_prefix(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut i = start;
    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    if i > start && i < chars.len() && chars[i] == ':' {
        Some((chars[start..i].iter().collect(), i + 1))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(negated: bool, field: Option<&str>, value: &str, quoted: bool) -> RawToken {
        RawToken {
            negated,
            field: field.map(str::to_owned),
            value: value.to_owned(),
            quoted,
        }
    }

    #[test]
    fn single_word() {
        assert_eq!(tokenize("foo"), vec![token(false, None, "foo", false)]);
    }

    #[test]
    fn multiple_words() {
        assert_eq!(
            tokenize("foo bar"),
            vec![
                token(false, None, "foo", false),
                token(false, None, "bar", false)
            ]
        );
    }

    #[test]
    fn quoted_phrase() {
        assert_eq!(
            tokenize(r#""foo bar""#),
            vec![token(false, None, "foo bar", true)]
        );
    }

    #[test]
    fn negated_word() {
        assert_eq!(tokenize("-foo"), vec![token(true, None, "foo", false)]);
    }

    #[test]
    fn lone_dash_is_a_word() {
        assert_eq!(tokenize("-"), vec![token(false, None, "-", false)]);
    }

    #[test]
    fn field_prefix_is_captured_verbatim() {
        assert_eq!(
            tokenize("artist:foo"),
            vec![token(false, Some("artist"), "foo", false)]
        );
    }

    #[test]
    fn negated_field_prefix() {
        assert_eq!(
            tokenize("-artist:foo"),
            vec![token(true, Some("artist"), "foo", false)]
        );
    }

    #[test]
    fn unknown_field_prefix_is_still_captured() {
        assert_eq!(
            tokenize("foo:bar"),
            vec![token(false, Some("foo"), "bar", false)]
        );
    }

    #[test]
    fn quoted_field_value() {
        assert_eq!(
            tokenize(r#"artist:"foo bar""#),
            vec![token(false, Some("artist"), "foo bar", true)]
        );
    }

    #[test]
    fn unclosed_quote_takes_rest_of_input() {
        assert_eq!(
            tokenize("abc \"def ghi"),
            vec![
                token(false, None, "abc", false),
                token(false, None, "def ghi", true)
            ]
        );
    }

    #[test]
    fn quote_in_middle_of_token() {
        assert_eq!(
            tokenize("abc\"def ghi\""),
            vec![token(false, None, "abcdef ghi", true)]
        );
    }

    #[test]
    fn empty_quotes_are_dropped() {
        assert_eq!(tokenize(r#""""#), vec![]);
    }

    #[test]
    fn field_with_empty_value_is_dropped() {
        assert_eq!(tokenize("artist:"), vec![]);
    }

    #[test]
    fn extra_whitespace_is_ignored() {
        assert_eq!(
            tokenize("  foo   bar  "),
            vec![
                token(false, None, "foo", false),
                token(false, None, "bar", false)
            ]
        );
    }
}
