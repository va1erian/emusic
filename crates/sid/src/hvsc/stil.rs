#![forbid(unsafe_code)]

//! Parser for HVSC's `STIL.txt` (SID Tune Information List).
//!
//! Each entry starts with an absolute HVSC path on its own line, followed by
//! indented fields (`NAME`, `AUTHOR`, `TITLE`, `ARTIST`, `COMMENT`). Multi-tune
//! SIDs split their fields into `(#N)` blocks. Only `COMMENT` text is kept;
//! continuation lines are appended to the current comment.

use std::collections::HashMap;

/// Parses STIL into a map of normalised HVSC-relative path -> comment text.
pub(super) fn parse(contents: &str) -> HashMap<String, String> {
    let mut entries = HashMap::new();
    let mut current: Option<Entry> = None;

    for line in contents.lines() {
        if line.starts_with('/') {
            flush(&mut entries, current.take());
            current = Some(Entry::new(line.trim().to_string()));
            continue;
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        if line.starts_with('#') {
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            entry.end_comment();
        } else if trimmed.starts_with("(#") {
            entry.subtune = parse_subtune(trimmed);
            entry.end_comment();
        } else if let Some((field, value)) = split_field(trimmed) {
            if field.eq_ignore_ascii_case("COMMENT") {
                entry.comment(value);
            } else {
                entry.end_comment();
            }
        } else {
            entry.continuation(trimmed);
        }
    }
    flush(&mut entries, current);

    entries
}

/// Inserts a finished entry, if it produced any comment text.
fn flush(entries: &mut HashMap<String, String>, entry: Option<Entry>) {
    if let Some(entry) = entry
        && let Some(comment) = entry.finish()
    {
        entries.insert(super::normalize(&entry.path), comment);
    }
}

/// Recognises the known STIL field labels (a colon in a continuation line is
/// not a field).
fn split_field(text: &str) -> Option<(&str, &str)> {
    const FIELDS: [&str; 5] = ["NAME", "AUTHOR", "TITLE", "ARTIST", "COMMENT"];
    let (field, value) = text.split_once(':')?;
    let field = field.trim();
    FIELDS
        .iter()
        .any(|known| field.eq_ignore_ascii_case(known))
        .then_some((field, value.trim()))
}

/// Parses a `(#N)` subtune marker.
fn parse_subtune(text: &str) -> Option<u16> {
    text.strip_prefix("(#")?
        .split_once(')')?
        .0
        .trim()
        .parse()
        .ok()
}

/// One STIL entry being parsed.
struct Entry {
    path: String,
    subtune: Option<u16>,
    blocks: Vec<(Option<u16>, String)>,
    active: Option<usize>,
}

impl Entry {
    fn new(path: String) -> Self {
        Self {
            path,
            subtune: None,
            blocks: Vec::new(),
            active: None,
        }
    }

    fn comment(&mut self, value: &str) {
        self.blocks.push((self.subtune, value.to_string()));
        self.active = Some(self.blocks.len() - 1);
    }

    fn continuation(&mut self, text: &str) {
        if let Some(index) = self.active
            && let Some(block) = self.blocks.get_mut(index)
        {
            if !block.1.is_empty() {
                block.1.push('\n');
            }
            block.1.push_str(text);
        }
    }

    fn end_comment(&mut self) {
        self.active = None;
    }

    fn finish(&self) -> Option<String> {
        let mut parts = Vec::new();
        for (subtune, text) in &self.blocks {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            match subtune {
                Some(number) => parts.push(format!("#{number}: {text}")),
                None => parts.push(text.to_string()),
            }
        }
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_simple_entry_with_a_wrapped_comment() {
        let text = "### /DEMOS/ ###\n\n/DEMOS/0-9/Tune.sid\nCOMMENT: First line\n\
                    \x20        second line\n\n/DEMOS/0-9/Other.sid\n  TITLE: X\n";
        let map = parse(text);
        assert_eq!(
            map.get("/demos/0-9/tune.sid").map(String::as_str),
            Some("First line\nsecond line")
        );
        assert!(!map.contains_key("/demos/0-9/other.sid"));
    }

    #[test]
    fn parses_subtune_comments() {
        let text = "/M/C/Tune.sid\nCOMMENT: global\n(#2)\nCOMMENT: second tune\n\
                    (#5)\nCOMMENT: fifth tune\n";
        let map = parse(text);
        assert_eq!(
            map.get("/m/c/tune.sid").map(String::as_str),
            Some("global\n\n#2: second tune\n\n#5: fifth tune")
        );
    }

    #[test]
    fn ignores_a_colon_in_a_continuation_line() {
        let text = "/a/Tune.sid\nCOMMENT: see http://example.com for details\n";
        let map = parse(text);
        assert_eq!(
            map.get("/a/tune.sid").map(String::as_str),
            Some("see http://example.com for details")
        );
    }
}
