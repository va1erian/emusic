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

use super::SettingsMsg;

/// The Playback page: session, tracker, MIDI and SID sections stacked.
pub(super) struct PlaybackPage {
    session: session::SessionSection,
    tracker: tracker::TrackerSection,
    midi: midi::MidiSection,
    sid: sid::SidSection,
}

impl PlaybackPage {
    /// Builds every section's controls.
    pub(super) fn new(ui: &mut Ui<Msg>, proxy: Proxy<Msg>) -> win32ui::Result<Self> {
        Ok(Self {
            session: session::SessionSection::new(ui)?,
            tracker: tracker::TrackerSection::new(ui)?,
            midi: midi::MidiSection::new(ui, proxy.clone())?,
            sid: sid::SidSection::new(ui, proxy)?,
        })
    }

    /// The page's controls as layout items, in display order.
    pub(super) fn items(&self) -> Vec<LayoutItem> {
        let mut items = self.session.items();
        items.extend(self.tracker.items());
        items.extend(self.midi.items());
        items.extend(self.sid.items());
        items
    }

    /// Shows or hides every control on the page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.session.set_visible(visible);
        self.tracker.set_visible(visible);
        self.midi.set_visible(visible);
        self.sid.set_visible(visible);
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
