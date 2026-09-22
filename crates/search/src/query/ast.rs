//! Abstract syntax tree produced by the parser.

/// A parsed search query: an ordered list of terms that must all match (AND).
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Terms in the order the user wrote them.
    pub terms: Vec<Term>,
}

/// One element of a parsed query.
#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    /// Whether this term must *not* match (`-` prefix).
    pub negated: bool,
    /// What the term matches against.
    pub body: TermBody,
}

impl Term {
    /// A positive text term matched against any field.
    #[must_use]
    pub fn text(value: TextMatch) -> Self {
        Self {
            negated: false,
            body: TermBody::Text(value),
        }
    }

    /// A negated text term matched against any field.
    #[must_use]
    pub fn negated_text(value: TextMatch) -> Self {
        Self {
            negated: true,
            body: TermBody::Text(value),
        }
    }

    /// A positive field-scoped term.
    #[must_use]
    pub fn field(field: Field, value: FieldValue) -> Self {
        Self {
            negated: false,
            body: TermBody::Field { field, value },
        }
    }

    /// A negated field-scoped term.
    #[must_use]
    pub fn negated_field(field: Field, value: FieldValue) -> Self {
        Self {
            negated: true,
            body: TermBody::Field { field, value },
        }
    }
}

/// What a [`Term`] matches against.
#[derive(Debug, Clone, PartialEq)]
pub enum TermBody {
    /// A bare word or phrase, matched against any field.
    Text(TextMatch),
    /// A field-scoped filter such as `artist:foo` or `year:1990..1999`.
    Field {
        /// The field the filter applies to.
        field: Field,
        /// The value to compare with.
        value: FieldValue,
    },
}

/// A piece of user text: either a single word or an exact phrase.
#[derive(Debug, Clone, PartialEq)]
pub enum TextMatch {
    /// A bare word, e.g. `foo`.
    Word(String),
    /// A quoted phrase, e.g. `"foo bar"`.
    Phrase(String),
}

impl TextMatch {
    /// A word match from any string-like value.
    #[must_use]
    pub fn word(value: impl Into<String>) -> Self {
        Self::Word(value.into())
    }

    /// A phrase match from any string-like value.
    #[must_use]
    pub fn phrase(value: impl Into<String>) -> Self {
        Self::Phrase(value.into())
    }

    /// The matched text, already normalized.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Word(text) | Self::Phrase(text) => text,
        }
    }
}

/// The value of a field-scoped filter.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// Text to match against the field.
    Text(TextMatch),
    /// A numeric comparison or range.
    Numeric(NumericSpec),
}

/// Queryable fields, both text and numeric.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Track artist.
    Artist,
    /// Album name.
    Album,
    /// Album artist.
    AlbumArtist,
    /// Track title.
    Title,
    /// Genre.
    Genre,
    /// File path or name.
    File,
    /// Containing directory.
    Dir,
    /// File extension.
    Ext,
    /// Comment tag.
    Comment,
    /// Composer tag.
    Composer,
    /// Release year.
    Year,
    /// Play count.
    Plays,
    /// Duration in seconds.
    Duration,
}

impl Field {
    /// Parses a field name as written by the user (case-insensitive).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "artist" => Some(Self::Artist),
            "album" => Some(Self::Album),
            "albumartist" | "album_artist" => Some(Self::AlbumArtist),
            "title" => Some(Self::Title),
            "genre" => Some(Self::Genre),
            "file" => Some(Self::File),
            "dir" | "directory" => Some(Self::Dir),
            "ext" | "extension" => Some(Self::Ext),
            "comment" => Some(Self::Comment),
            "composer" => Some(Self::Composer),
            "year" => Some(Self::Year),
            "plays" | "playcount" => Some(Self::Plays),
            "duration" => Some(Self::Duration),
            _ => None,
        }
    }

    /// Whether the field holds a number rather than text.
    #[must_use]
    pub const fn is_numeric(self) -> bool {
        matches!(self, Self::Year | Self::Plays | Self::Duration)
    }
}

/// A comparison operator for a single numeric value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Comparison {
    /// `<`
    Less,
    /// `<=`
    LessEqual,
    /// `=` (also the implicit operator for a bare value)
    Equal,
    /// `>=`
    GreaterEqual,
    /// `>`
    Greater,
}

/// A numeric filter: a single comparison or an inclusive range.
///
/// Ranges may leave either bound open, e.g. `year:..1999` or `year:1990..`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumericSpec {
    /// A single value with an operator, e.g. `year:1994` or `plays:>10`.
    Compare {
        /// The comparison operator.
        op: Comparison,
        /// The value to compare against.
        value: f64,
    },
    /// An inclusive range, e.g. `year:1990..1999`.
    Range {
        /// Lower bound, if present.
        min: Option<f64>,
        /// Upper bound, if present.
        max: Option<f64>,
    },
}
