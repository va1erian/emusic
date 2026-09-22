//! Shared UI state, the [`View`] router, [`Theme`], [`Accent`] and the
//! [`Command`] message type panels/views use to ask the shell to change
//! something.

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::views::track_table::TrackTableState;

#[cfg(test)]
mod tests;

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

/// The UI accent colour: a built-in preset or a custom colour picked in
/// Settings → Appearance (#40).
///
/// Serializes as the preset's lowercase name (`"blue"`) or a `#rrggbb`
/// string for custom colours, so the config file stays human-editable and
/// the same spellings work for `emusic-shot --accent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Accent {
    #[default]
    Orange,
    Blue,
    Green,
    Purple,
    Red,
    Teal,
    /// Any colour chosen with the custom picker.
    Custom(Color32),
}

impl Accent {
    /// The built-in presets, in display order.
    pub const PRESETS: [Self; 6] = [
        Self::Orange,
        Self::Blue,
        Self::Green,
        Self::Purple,
        Self::Red,
        Self::Teal,
    ];

    /// Label shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Orange => "Orange",
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Purple => "Purple",
            Self::Red => "Red",
            Self::Teal => "Teal",
            Self::Custom(_) => "Custom",
        }
    }

    /// The colour to render with.
    pub fn color(self) -> Color32 {
        match self {
            Self::Orange => crate::theme::DEFAULT_ACCENT,
            Self::Blue => Color32::from_rgb(0x35, 0x84, 0xE4),
            Self::Green => Color32::from_rgb(0x2E, 0xC2, 0x7E),
            Self::Purple => Color32::from_rgb(0x91, 0x41, 0xAC),
            Self::Red => Color32::from_rgb(0xE0, 0x1B, 0x24),
            Self::Teal => Color32::from_rgb(0x0F, 0x9B, 0xA0),
            Self::Custom(color) => color,
        }
    }

    /// Parses the config/CLI spelling: a preset name (case-insensitive) or
    /// a `#rrggbb` hex colour.
    pub fn parse(s: &str) -> Option<Self> {
        let lowered = s.trim().to_ascii_lowercase();
        if let Some(preset) = Self::PRESETS
            .into_iter()
            .find(|preset| preset.label().to_ascii_lowercase() == lowered)
        {
            return Some(preset);
        }
        parse_hex(s).map(Self::Custom)
    }

    /// The canonical config/CLI spelling. [`Self::parse`] also accepts
    /// preset names in any case.
    pub fn to_config_str(self) -> String {
        match self {
            Self::Custom(color) => {
                format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
            }
            other => other.label().to_ascii_lowercase(),
        }
    }
}

/// `#rrggbb` (with the `#` optional) to an opaque colour.
fn parse_hex(s: &str) -> Option<Color32> {
    let trimmed = s.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    Some(Color32::from_rgb(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ))
}

impl Serialize for Accent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_config_str())
    }
}

impl<'de> Deserialize<'de> for Accent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown accent {s:?}; expected a preset name or #rrggbb"
            ))
        })
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
    SetAccent(Accent),
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
    pub accent: Accent,
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
            accent: Accent::default(),
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
            Command::SetAccent(accent) => self.accent = *accent,
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
