//! [`SettingsTab`]: which Settings sub-page (#137) the shell is showing.
//! Splitting the page into tabs keeps a long library-folder list from pushing
//! the Appearance and File association sections off-screen. Transient UI
//! state, so it is not persisted to the config.

/// A sub-page of the Settings view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    /// Configured library folders and the controls to add, remove or rescan
    /// them (#19).
    #[default]
    Library,
    /// Theme and accent colour (#40).
    Appearance,
    /// Which file types open with emusic (#11).
    Associations,
    /// Tracker module playback options (interpolation, ramping, emulation,
    /// ...).
    Playback,
    /// Version, revision and credits (#188).
    About,
}

impl SettingsTab {
    pub const ALL: [Self; 5] = [
        Self::Library,
        Self::Appearance,
        Self::Associations,
        Self::Playback,
        Self::About,
    ];

    /// Label shown on the Settings tab strip.
    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Appearance => "Appearance",
            Self::Associations => "File associations",
            Self::Playback => "Tracker playback",
            Self::About => "About",
        }
    }

    /// CLI-friendly identifier, e.g. for `emusic-shot --settings-tab`.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Appearance => "appearance",
            Self::Associations => "associations",
            Self::Playback => "playback",
            Self::About => "about",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.slug() == slug)
    }
}
