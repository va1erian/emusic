//! Shared UI state, the [`View`] router, [`Theme`] and the [`Command`]
//! message type panels/views use to ask the shell to change something.

use serde::{Deserialize, Serialize};

use crate::views::track_table::TrackTableState;

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
    MostPlayed,
    History,
    NowPlaying,
    Settings,
}

impl View {
    pub const ALL: [Self; 9] = [
        Self::Music,
        Self::Albums,
        Self::Artists,
        Self::Genres,
        Self::Folders,
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

/// Colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

/// Which optional panels are visible (toggled from the View menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelVisibility {
    pub navigator: bool,
    pub right_panel: bool,
    pub status_bar: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            navigator: true,
            right_panel: true,
            status_bar: true,
        }
    }
}

/// One-shot request emitted by a panel/view during `ui()`, applied by the
/// shell after layout so widgets never need `&mut AppState` themselves.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SetView(View),
    ToggleTheme,
    TogglePanel(PanelKind),
    SetSearchQuery(String),
    PlayerPlayPause,
    PlayerStop,
    PlayerNext,
    PlayerPrevious,
    PlayerSeek(std::time::Duration),
    PlayerSetVolume(f32),
    PlayerToggleRepeat,
    PlayerToggleShuffle,
    /// Start playing this track. A stand-in for real queue control (#4):
    /// currently a no-op in the shell, kept here so the track table's
    /// double-click/Enter/context menu have somewhere to send intent.
    PlayTrack(u64),
    /// "Play next" from a track's context menu; same caveat as
    /// [`Command::PlayTrack`].
    PlayTrackNext(u64),
    /// "Add to queue" from a track's context menu; same caveat as
    /// [`Command::PlayTrack`].
    QueueTrack(u64),
    /// Jump to a queue entry by its current index and start playback.
    PlayerQueueJump(usize),
    /// Remove a queue entry by its current index.
    PlayerQueueRemove(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Navigator,
    RightPanel,
    StatusBar,
}

/// Everything the shell needs beyond the player/library data itself.
pub struct AppState {
    pub view: View,
    pub theme: Theme,
    pub panels: PanelVisibility,
    pub search_query: String,
    /// The Music view's track table (sort + selection). Other views that
    /// embed a track table later (albums, artists, genres, folders,
    /// history) will each get their own field here.
    pub music_table: TrackTableState,
    /// Commands queued during this frame's `ui()`, drained at the end.
    pub pending: Vec<Command>,
    /// Persistent state for the right-hand now-playing panel (artwork cache,
    /// collapsible section flags, ...).
    pub now_playing: crate::panels::now_playing::PanelState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            view: View::Music,
            theme: Theme::Dark,
            panels: PanelVisibility::default(),
            search_query: String::new(),
            music_table: TrackTableState::default(),
            pending: Vec::new(),
            now_playing: crate::panels::now_playing::PanelState::default(),
        }
    }
}

impl AppState {
    pub fn push(&mut self, cmd: Command) {
        self.pending.push(cmd);
    }

    /// Applies every queued command to the local (non-player, non-library)
    /// bits of state. Player/library commands are applied by the caller,
    /// which owns those trait objects.
    pub fn apply_local(&mut self, cmd: &Command) {
        match cmd {
            Command::SetView(view) => self.view = *view,
            Command::ToggleTheme => self.theme = self.theme.toggled(),
            Command::TogglePanel(kind) => match kind {
                PanelKind::Navigator => self.panels.navigator = !self.panels.navigator,
                PanelKind::RightPanel => self.panels.right_panel = !self.panels.right_panel,
                PanelKind::StatusBar => self.panels.status_bar = !self.panels.status_bar,
            },
            Command::SetSearchQuery(query) => self.search_query = query.clone(),
            _ => {}
        }
    }
}
