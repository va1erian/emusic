//! Shared UI state, the [`View`] router, [`Theme`] and the [`Command`]
//! message type panels/views use to ask the shell to change something.

/// Which central-area view is currently shown.
///
/// New views (track table, album grid, column browser, ...) add a variant
/// here plus a module under `views/`; the router in `views::show` is the
/// only other place that needs updating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum View {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Commands queued during this frame's `ui()`, drained at the end.
    pub pending: Vec<Command>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            view: View::Music,
            theme: Theme::Dark,
            panels: PanelVisibility::default(),
            search_query: String::new(),
            pending: Vec::new(),
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
