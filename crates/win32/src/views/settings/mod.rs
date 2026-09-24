//! Win32 Settings view (#115).
//!
//! A view struct per the #106 pattern: a native tab strip (`win32ui`'s
//! [`Tabs`](win32ui::Tabs) control) over five pages that mirror the egui
//! Settings view — Library folders, Appearance, File associations, Playback
//! and About. Each page owns standard win32ui controls and only reads and
//! writes the shared `emusic-ui` state, emitting [`Command`]s for the shell to
//! apply; it never duplicates sorting, filtering or formatting.
//!
//! [`SettingsView::tabs`] rebuilds the tab node each time the window layout is
//! installed, so the strip only exists while Settings is the active view; the
//! Library page's folder list is a native `ListView` with its own scrollbar.
//!
//! Each page's controls are created through its [`ScrollPanel`] — a `win32ui`
//! `Panel` inside a `ScrollView` (win32ui #119) — so a page taller than the
//! window scrolls vertically with the wheel and the scrollbar instead of being
//! cut off.

mod about;
mod appearance;
mod associations;
mod library;
mod playback;
mod scroll_form;

use scroll_form::{FormRow, ScrollPanel};

use std::cell::Cell;
use std::path::PathBuf;

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::state::{Accent, AppState, SettingsTab, Theme as UiTheme, VisualizerMode};
use emusic_ui::views::Commands;
use win32ui::prelude::*;
use win32ui::{Layout, dip};

use crate::app::Msg;

/// Height of a form row (label + field), in design units.
pub(super) const ROW_HEIGHT: f32 = 28.0;
/// Width of the label column in a form row, in design units.
pub(super) const LABEL_WIDTH: f32 = 160.0;
/// Height of a page's section heading, in design units.
pub(super) const HEADING_HEIGHT: f32 = 24.0;

/// Every intent the Settings view can raise, mapped from its controls.
pub enum SettingsMsg {
    /// The tab strip selected another page.
    SelectTab(SettingsTab),
    /// Open the native folder picker (Library page).
    AddFolder,
    /// The folder picker returned (Library page).
    FolderPicked(Option<PathBuf>),
    /// Remove the folder selected in the list.
    RemoveFolder,
    /// Rescan every configured folder.
    Rescan,
    /// Stop the scan currently running.
    CancelScan,
    /// The Appearance page picked a colour scheme.
    SetTheme(UiTheme),
    /// The Appearance page picked an accent preset.
    SetAccent(Accent),
    /// The Appearance page toggled the status-bar visualizer.
    ToggleVisualizer(bool),
    /// The Appearance page picked a visualizer mode.
    SetVisualizerMode(VisualizerMode),
    /// The File associations page toggled one extension's checkbox.
    AssocToggle(usize, bool),
    /// The File associations page selected or cleared every checkbox.
    AssocSelect(bool),
    /// Register the selected extensions (and open Windows Default apps).
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
    Tracker(playback::TrackerEdit),
    /// The Playback page applied a tracker preset.
    TrackerPreset(playback::TrackerPreset),
    /// Open the native soundfont file picker.
    MidiBrowse,
    /// The soundfont picker returned.
    MidiPicked(Option<PathBuf>),
    /// Commit the soundfont path typed into the text field.
    MidiCommit,
    /// Clear the MIDI soundfont.
    MidiClear,
    /// Open the native file picker for the Songlengths database.
    SidBrowseFile,
    /// Open the native folder picker for an HVSC root.
    SidBrowseFolder,
    /// The Songlengths picker returned.
    SidPicked(Option<PathBuf>),
    /// Commit the Songlengths path typed into the text field.
    SidCommit,
    /// Clear the Songlengths database path.
    SidClear,
    /// The SID fallback play length slider moved.
    SidFallback(u32),
}

/// The Settings central area: the five pages (the tab strip is built fresh by
/// [`SettingsView::tabs`] whenever the window layout is installed).
pub struct SettingsView {
    library: library::LibraryPage,
    appearance: appearance::AppearancePage,
    associations: associations::AssociationsPage,
    playback: playback::PlaybackPage,
    about: about::AboutPage,
    /// Whether the whole view is currently shown; the selected page is only
    /// visible when this is set too.
    visible: Cell<bool>,
    /// The tab the page visibility was last applied for.
    applied_tab: SettingsTab,
}

impl SettingsView {
    /// Builds every page's controls.
    pub fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let proxy = ui.proxy();
        let library = library::LibraryPage::new(ui, proxy.clone())?;
        let appearance = appearance::AppearancePage::new(ui)?;
        let associations = associations::AssociationsPage::new(ui)?;
        let playback = playback::PlaybackPage::new(ui, proxy)?;
        let about = about::AboutPage::new(ui)?;

        let view = Self {
            library,
            appearance,
            associations,
            playback,
            about,
            visible: Cell::new(false),
            applied_tab: SettingsTab::Library,
        };
        view.refresh_tab_visibility();
        Ok(view)
    }

    /// The Settings central area as a native tab control with one page per
    /// settings tab, initially showing the active one.
    ///
    /// The node is rebuilt on every layout install, so the returned value is
    /// only handed straight to `Ui::set_layout`.
    #[must_use]
    pub fn tabs(&self) -> Tabs {
        Tabs::new()
            .page(SettingsTab::Library.label(), self.library.page())
            .page(SettingsTab::Appearance.label(), self.appearance.page())
            .page(SettingsTab::Associations.label(), self.associations.page())
            .page(SettingsTab::Playback.label(), self.playback.page())
            .page(SettingsTab::About.label(), self.about.page())
            .initial(
                SettingsTab::ALL
                    .iter()
                    .position(|tab| *tab == self.applied_tab)
                    .unwrap_or(0),
            )
            .on_change(|index| {
                SettingsTab::ALL
                    .get(index)
                    .map(|tab| Msg::Settings(SettingsMsg::SelectTab(*tab)))
            })
    }

    /// Shows or hides the whole view (every page).
    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        self.refresh_tab_visibility();
    }

    /// Mirrors the active tab onto the five pages, leaving only the selected
    /// one visible (when the view itself is shown).
    fn refresh_tab_visibility(&self) {
        let shown = self.visible.get();
        let tab = self.applied_tab;
        self.library
            .set_visible(shown && tab == SettingsTab::Library);
        self.appearance
            .set_visible(shown && tab == SettingsTab::Appearance);
        self.associations
            .set_visible(shown && tab == SettingsTab::Associations);
        self.playback
            .set_visible(shown && tab == SettingsTab::Playback);
        self.about.set_visible(shown && tab == SettingsTab::About);
    }

    /// Pushes the shell state into the pages.
    ///
    /// Returns whether the active tab changed, so the caller can reinstall the
    /// window layout and build the tab control on the new page (the node has no
    /// runtime "select" setter).
    pub fn sync(
        &mut self,
        ui: &Ui<Msg>,
        state: &AppState,
        library: &dyn LibraryDataSource,
    ) -> bool {
        let tab_changed = state.settings_tab != self.applied_tab;
        if tab_changed {
            self.applied_tab = state.settings_tab;
            self.refresh_tab_visibility();
        }
        self.library.sync(state, library);
        self.appearance.sync(ui, state);
        self.associations.sync();
        self.playback.sync(state);
        tab_changed
    }

    /// Applies one control intent, queueing the [`Command`]s it raises.
    pub fn update(
        &mut self,
        msg: SettingsMsg,
        ui: &mut Ui<Msg>,
        state: &mut AppState,
        out: &mut Commands,
    ) {
        if let SettingsMsg::SelectTab(tab) = msg {
            // The tab control already shows the new page; `sync` notices the
            // change and reinstalls the layout around it.
            state.settings_tab = tab;
            return;
        }

        if self.library.update(&msg, out) {
            return;
        }
        if self.appearance.update(&msg, ui, state, out) {
            return;
        }
        if self.associations.update(&msg) {
            return;
        }
        self.playback.update(&msg, state, out);
    }
}

/// Lays a [`RadioGroup`]'s options out in a single horizontal row (used by the
/// Appearance and Playback forms for their small choice fields).
pub(super) fn radio_row<T: 'static, M: 'static>(group: &RadioGroup<T, M>) -> Layout {
    let mut row = Layout::row().spacing(dip(16.0));
    for option in group.options() {
        row = row.item(option);
    }
    row
}

/// Builds a "label: field" form row of the shared height.
pub(super) fn labelled(label: &Label, field: LayoutItem) -> LayoutItem {
    row![label.width(dip(LABEL_WIDTH)), field].height(dip(ROW_HEIGHT))
}
