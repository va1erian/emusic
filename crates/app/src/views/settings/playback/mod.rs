//! Settings → Playback page (#190, #115): session resume, tracker module
//! settings, the MIDI soundfont and the SID song-length database.
//!
//! The page is a thin composition of four independent sections, each owning its
//! controls and mirroring one area of the shared state. Everything is applied
//! through the shared [`Command`](emusic_ui::state::Command)s.

mod midi;
mod session;
mod sid;
mod tracker;

pub use tracker::{TrackerEdit, TrackerPreset};

use std::path::{Path, PathBuf};

use emusic_ui::state::AppState;
use emusic_ui::views::Commands;
use xui::xui_core::app::Ui;

use super::SettingsMsg;
use super::form::FormPage;
use crate::app::Msg;

/// The Playback page: session, tracker, MIDI and SID sections stacked.
pub(super) struct PlaybackPage {
    form: FormPage,
    session: session::SessionSection,
    tracker: tracker::TrackerSection,
    midi: midi::MidiSection,
    sid: sid::SidSection,
}

impl PlaybackPage {
    /// Builds every section's controls.
    pub(super) fn new(ui: &Ui<Msg>) -> PlaybackPage {
        let form = FormPage::new(ui).expect("create playback form");
        let session = session::SessionSection::new(&form);
        let tracker = tracker::TrackerSection::new(&form);
        let midi = midi::MidiSection::new(&form, ui.proxy());
        let sid = sid::SidSection::new(&form, ui.proxy());
        PlaybackPage {
            form,
            session,
            tracker,
            midi,
            sid,
        }
    }

    /// The page's node identity (its scroll view).
    pub(super) fn id(&self) -> xui::xui_core::backend::WidgetId {
        self.form.id()
    }

    /// Re-lays the page's form.
    pub(super) fn relayout(&self) {
        self.form.relayout();
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.form.set_visible(visible);
    }

    /// Pushes the shared state onto every section's controls.
    pub(super) fn sync(&mut self, state: &AppState) {
        self.session.sync(state);
        self.tracker.sync(state);
        self.midi.sync(state);
        self.sid.sync(state);
    }

    /// Handles the Playback page's messages; returns whether `msg` was one.
    pub(super) fn update(
        &mut self,
        msg: &SettingsMsg,
        state: &mut AppState,
        out: &mut Commands,
    ) -> bool {
        self.session.update(msg, state)
            || self.tracker.update(msg, state, out)
            || self.midi.update(msg, out)
            || self.sid.update(msg, out)
    }
}

/// The display text for a path, or empty when unset.
pub(super) fn path_text(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_default()
}

/// Trims a typed path and turns an empty one into `None`.
pub(super) fn parse(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim().trim_matches('"');
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}
