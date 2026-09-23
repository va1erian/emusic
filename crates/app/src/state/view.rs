//! [`View`]: which central-area view the shell is currently showing.

use serde::{Deserialize, Serialize};

/// Which central-area view is currently shown.
///
/// New views (track table, album grid, column browser, ...) add a variant
/// here plus a module under `views/`; the router in `views::show` is the
/// only other place that needs updating.
///
/// Serde uses the same kebab-case identifiers as [`View::slug`], so the
/// config file's `last_view` matches the CLI/`emusic-shot` spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum View {
    #[default]
    Music,
    Albums,
    Artists,
    Genres,
    Folders,
    Starred,
    MostPlayed,
    History,
    NowPlaying,
    Settings,
}

impl View {
    pub const ALL: [Self; 10] = [
        Self::Music,
        Self::Albums,
        Self::Artists,
        Self::Genres,
        Self::Folders,
        Self::Starred,
        Self::MostPlayed,
        Self::History,
        Self::NowPlaying,
        Self::Settings,
    ];

    /// Short label used in the navigator and window title.
    pub fn label(self) -> &'static str {
        match self {
            Self::Music => "Music",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::Genres => "Genres",
            Self::Folders => "Folders",
            Self::Starred => "Starred",
            Self::MostPlayed => "Most Played",
            Self::History => "History",
            Self::NowPlaying => "Now Playing",
            Self::Settings => "Settings",
        }
    }

    /// CLI-friendly identifier, e.g. for `emusic-shot --view most-played`.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Albums => "albums",
            Self::Artists => "artists",
            Self::Genres => "genres",
            Self::Folders => "folders",
            Self::Starred => "starred",
            Self::MostPlayed => "most-played",
            Self::History => "history",
            Self::NowPlaying => "now-playing",
            Self::Settings => "settings",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.slug() == slug)
    }
}
