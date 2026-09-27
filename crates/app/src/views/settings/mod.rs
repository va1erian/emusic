//! The Settings view (#115, #400): a portable [`Tabs`] strip over the six
//! pages the Win32 build had — Library folders, Appearance, Visualization,
//! File associations, Playback and About.
//!
//! Each page owns its portable widgets and only reads and writes the shared
//! `emusic-ui` state, emitting [`Command`]s for the shell to apply; this module
//! wires them into the tab strip and routes the page [`SettingsMsg`]s. Every
//! page scrolls with its own [`ScrollView`](form::FormPage), so a form taller
//! than the window scrolls instead of being cut off.
//!
//! The tab strip itself owns page visibility: only the selected page's child is
//! shown. `SettingsView` mirrors the shared [`SettingsTab`] onto it and re-lays
//! the selected page's form after the strip has positioned it.

pub mod about;
pub mod accent_swatches;
mod appearance;
mod associations;
mod form;
mod library;
mod playback;
mod visualization;

use std::cell::Cell;
use std::path::PathBuf;

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::state::{AppState, Density, FontSize, SettingsTab, Theme, VisualizerMode};
use emusic_ui::views::Commands;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::Tabs;

use crate::app::Msg;

pub use playback::{TrackerEdit, TrackerPreset};
pub use visualization::VisualizationEdit;

/// Everything the Settings controls can raise, mapped onto the shared state and
/// [`Command`](emusic_ui::state::Command)s.
pub enum SettingsMsg {
    /// The tab strip selected another page.
    SelectTab(SettingsTab),
    /// The Library page's folder list selection changed (drives Remove).
    LibrarySelect,
    /// Open the folder picker to add a library folder.
    AddFolder,
    /// Remove the folder selected in the list.
    RemoveFolder,
    /// Rescan every configured folder.
    Rescan,
    /// Stop the running scan.
    CancelScan,
    /// The Appearance page picked a colour scheme.
    SetTheme(Theme),
    /// The Appearance page toggled the window accent tint (#355).
    SetAccentTint(bool),
    /// The Appearance page changed the accent tint strength (#355).
    SetAccentTintStrength(u8),
    /// The Appearance page toggled the status-bar visualizer.
    ToggleVisualizer(bool),
    /// The Appearance page picked a visualizer mode.
    SetVisualizerMode(VisualizerMode),
    /// The Appearance page picked a UI font size (#309).
    SetFontSize(FontSize),
    /// The Appearance page picked a list density (#309).
    SetDensity(Density),
    /// The Appearance page toggled zebra striping (#309).
    ToggleZebra(bool),
    /// The Visualization page changed a settings field (#306).
    Viz(VisualizationEdit),
    /// The Visualization page toggled a preset pack (#306).
    VizPack(String, bool),
    /// Launch the preset-setup helper (#306).
    VizGetPresets,
    /// Open the folder picker for the user preset folder (#306).
    VizBrowse,
    /// The user preset folder picker returned (#306).
    VizPicked(Option<PathBuf>),
    /// Commit the user preset path typed into the field (#306).
    VizCommitUserDir,
    /// Clear the user preset folder (#306).
    VizClearUserDir,
    /// The background preset-pack count finished (#306).
    VizPackCounts(Vec<(String, usize)>),
    /// The Associations page toggled one extension's checkbox.
    AssocToggle(usize, bool),
    /// The Associations page selected or cleared every checkbox.
    AssocSelect(bool),
    /// Register the selected extensions and open Windows Default apps.
    AssocRegister,
    /// Remove every registered extension.
    AssocUnregister,
    /// Open Windows' own Default apps settings page.
    AssocOpenSettings,
    /// The Playback page toggled session resume.
    ResumePlayback(bool),
    /// The Playback page toggled autoplay-on-restore.
    AutoplayOnRestore(bool),
    /// The Playback page changed one tracker setting.
    Tracker(TrackerEdit),
    /// The Playback page applied a tracker preset.
    TrackerPreset(TrackerPreset),
    /// Open the soundfont file picker.
    MidiBrowse,
    /// The soundfont picker returned.
    MidiPicked(Option<PathBuf>),
    /// Commit the soundfont path typed into the field.
    MidiCommit,
    /// Clear the MIDI soundfont.
    MidiClear,
    /// Open the file picker for the Songlengths database.
    SidBrowseFile,
    /// Open the folder picker for an HVSC root.
    SidBrowseFolder,
    /// The Songlengths picker returned.
    SidPicked(Option<PathBuf>),
    /// Commit the Songlengths path typed into the field.
    SidCommit,
    /// Clear the Songlengths database path.
    SidClear,
    /// The SID fallback play length slider moved.
    SidFallback(u32),
}

/// The Settings central area: a tab strip over the six pages.
pub struct SettingsView {
    ui: Ui<Msg>,
    tabs: Tabs<Msg>,
    library: library::LibraryPage,
    appearance: appearance::AppearancePage,
    visualization: visualization::VisualizationPage,
    associations: associations::AssociationsPage,
    playback: playback::PlaybackPage,
    about: xui::xui_core::widget::FlowText<Msg>,
    /// Whether the whole view is shown; the selected page is only visible when
    /// this is set too.
    visible: Cell<bool>,
    /// The tab whose page has been laid out. Tracked here rather than read from
    /// the strip because a strip click selects the tab itself before it raises
    /// [`Msg::Settings`], so the strip would already match the shell state by
    /// the time [`Self::sync`] runs.
    applied_tab: Cell<SettingsTab>,
}

impl SettingsView {
    /// Builds every page and the tab strip over them.
    pub fn new(ui: &Ui<Msg>) -> SettingsView {
        let tabs = Tabs::new(ui, Rect::default()).expect("create settings tab strip");
        let inner = tabs.ui().clone();
        let library = library::LibraryPage::new(&inner);
        let appearance = appearance::AppearancePage::new(&inner);
        let visualization = visualization::VisualizationPage::new(&inner);
        let associations = associations::AssociationsPage::new(&inner);
        let playback = playback::PlaybackPage::new(&inner);
        let about = about::build(&inner, Rect::default());
        let tabs = tabs
            .page(SettingsTab::Library.label(), &[library.id()])
            .page(SettingsTab::Appearance.label(), &[appearance.id()])
            .page(SettingsTab::Visualization.label(), &[visualization.id()])
            .page(SettingsTab::Associations.label(), &[associations.id()])
            .page(SettingsTab::Playback.label(), &[playback.id()])
            .page(SettingsTab::About.label(), &[about.id()])
            .on_change(|index| {
                SettingsTab::ALL
                    .get(index)
                    .copied()
                    .map(|tab| Msg::Settings(SettingsMsg::SelectTab(tab)))
            });

        SettingsView {
            ui: ui.clone(),
            tabs,
            library,
            appearance,
            visualization,
            associations,
            playback,
            about,
            visible: Cell::new(false),
            applied_tab: Cell::new(SettingsTab::default()),
        }
    }

    /// Moves/resizes the view and re-lays the selected page's form.
    pub fn set_bounds(&self, rect: Rect) {
        self.tabs.set_bounds(rect);
        self.relayout_pages();
    }

    /// Shows or hides the whole view (every page).
    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        self.tabs.set_visible(visible);
        self.apply_page_visibility();
        self.relayout_pages();
    }

    /// Pushes the shell state into the pages, selecting the shared tab and
    /// re-laying the page when it changed.
    pub fn sync(&mut self, state: &AppState, library: &dyn LibraryDataSource) {
        if state.settings_tab != self.applied_tab.get() {
            let index = SettingsTab::ALL
                .iter()
                .position(|tab| *tab == state.settings_tab)
                .unwrap_or(0);
            self.tabs.select(index);
            self.applied_tab.set(state.settings_tab);
            // Show the page before re-laying its form out: a hidden page has no
            // bounds, so placing its widgets would be a no-op and they would
            // stay at creation size until the next resize.
            self.apply_page_visibility();
            self.relayout_pages();
        }
        self.library.sync(state, library);
        self.appearance.sync(state);
        self.visualization.sync(state);
        self.associations.sync();
        self.playback.sync(state);
    }

    /// Applies one control intent, queueing the [`Command`]s it raises.
    pub fn update(&mut self, msg: SettingsMsg, state: &mut AppState, out: &mut Commands) {
        if let SettingsMsg::SelectTab(tab) = msg {
            state.settings_tab = tab;
            return;
        }
        if self.library.update(&msg, out) {
            return;
        }
        if self.appearance.update(&msg, state, out) {
            return;
        }
        if self.visualization.update(&msg, state, out) {
            return;
        }
        if self.associations.update(&msg) {
            return;
        }
        self.playback.update(&msg, state, out);
    }

    /// Re-lays every page's form from the bounds the tab strip gave it.
    fn relayout_pages(&self) {
        self.library.relayout();
        self.appearance.relayout();
        self.visualization.relayout();
        self.associations.relayout();
        self.playback.relayout();
    }

    /// Shows only the selected page, and only while the view is shown.
    ///
    /// The tab strip already applies this on its own re-layouts, but the
    /// portable backends do not hide a hidden parent's children, so re-apply it
    /// here (the order matches [`SettingsTab::ALL`]).
    fn apply_page_visibility(&self) {
        let shown = self.visible.get();
        let selected = self.tabs.selected();
        let vis = |index: usize| shown && index == selected;
        self.library.set_visible(vis(0));
        self.appearance.set_visible(vis(1));
        self.visualization.set_visible(vis(2));
        self.associations.set_visible(vis(3));
        self.playback.set_visible(vis(4));
        self.ui.set_visible(self.about.id(), vis(5));
    }
}
