//! Playback page → session resume (#190, #214): whether the last played track
//! and queue come back on startup, and whether a restored session starts
//! playing. Persisted plain state, set directly.

use emusic_ui::state::AppState;
use win32ui::CheckBox;
use win32ui::prelude::*;

use crate::app::Msg;

use super::super::{FormRow, ROW_HEIGHT, SettingsMsg};

/// The session-resume checkboxes.
pub(super) struct SessionSection {
    resume: CheckBox<Msg>,
    autoplay: CheckBox<Msg>,
}

impl SessionSection {
    /// Builds the two checkboxes.
    pub(super) fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        Ok(Self {
            resume: CheckBox::new(ui, "Resume playback on startup")?
                .on_toggle(|on| Some(Msg::Settings(SettingsMsg::ResumePlayback(on)))),
            autoplay: CheckBox::new(ui, "Start playing when resuming")?
                .on_toggle(|on| Some(Msg::Settings(SettingsMsg::AutoplayOnRestore(on)))),
        })
    }

    /// The section's controls as form rows.
    pub(super) fn rows(&self) -> Vec<FormRow> {
        vec![
            (self.resume.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (self.autoplay.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
        ]
    }

    /// Mirrors the shared state onto the checkboxes.
    pub(super) fn sync(&self, state: &AppState) {
        self.resume.set_checked(state.resume_playback);
        self.autoplay.set_checked(state.autoplay_on_restore);
        self.autoplay.set_enabled(state.resume_playback);
    }

    /// Handles the session-resume messages.
    pub(super) fn update(&self, msg: &SettingsMsg, state: &mut AppState) -> bool {
        match msg {
            SettingsMsg::ResumePlayback(on) => state.resume_playback = *on,
            SettingsMsg::AutoplayOnRestore(on) => state.autoplay_on_restore = *on,
            _ => return false,
        }
        true
    }
}
