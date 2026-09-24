//! Settings → Playback page (#190, #115): session resume, tracker module
//! settings, the MIDI soundfont and the SID song-length database.
//!
//! The page is a thin composition of four independent sections, each owning
//! its controls and mirroring one area of the shared state. Everything is
//! applied through the same [`Command`](emusic_ui::state::Command)s the egui
//! Playback tab raises.

mod midi;
mod session;
mod sid;
mod tracker;

pub(super) use tracker::{TrackerEdit, TrackerPreset};

use std::path::{Path, PathBuf};

use emusic_ui::state::AppState;
use emusic_ui::views::Commands;
use win32ui::Proxy;
use win32ui::prelude::*;

use crate::app::Msg;

use super::{FormRow, ScrollPanel, SettingsMsg};

/// The Playback page: session, tracker, MIDI and SID sections stacked.
pub(super) struct PlaybackPage {
    form: ScrollPanel,
    session: session::SessionSection,
    tracker: tracker::TrackerSection,
    midi: midi::MidiSection,
    sid: sid::SidSection,
}

impl PlaybackPage {
    /// Builds every section's controls.
    pub(super) fn new(ui: &mut Ui<Msg>, proxy: Proxy<Msg>) -> win32ui::Result<Self> {
        let form = ScrollPanel::new(ui)?;
        let mut panel = form.ui(ui);
        let page = Self {
            session: session::SessionSection::new(&mut panel)?,
            tracker: tracker::TrackerSection::new(&mut panel)?,
            midi: midi::MidiSection::new(&mut panel, proxy.clone())?,
            sid: sid::SidSection::new(&mut panel, proxy)?,
            form,
        };
        page.apply(ui);
        Ok(page)
    }

    /// The page's scrollable form as one tab-strip page.
    pub(super) fn page(&self) -> LayoutItem {
        self.form.page()
    }

    /// The page's controls as form rows, in display order.
    fn rows(&self) -> Vec<FormRow> {
        let mut rows = self.session.rows();
        rows.extend(self.tracker.rows());
        rows.extend(self.midi.rows());
        rows.extend(self.sid.rows());
        rows
    }

    /// Reinstalls the page's form (used after a visibility change).
    fn apply(&self, ui: &Ui<Msg>) {
        self.form.apply(ui, self.rows());
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
