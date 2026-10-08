//! Playback page → session resume (#190, #214): whether the last played track
//! and queue come back on startup, and whether a restored session starts
//! playing. Persisted plain state, set directly.

use emusic_ui::state::AppState;
use xui::xui_core::widget::CheckBox;

use super::super::SettingsMsg;
use super::super::form::{FormPage, HEADING_HEIGHT, ROW_HEIGHT};
use crate::app::Msg;
use crate::make::make;
use xui::xui_core::arrange::{checkbox, label};

/// The session-resume section's controls.
pub(super) struct SessionSection {
    /// Held so the heading's node stays alive (dropping a widget destroys it).
    _heading: xui::xui_core::widget::Label<Msg>,
    resume: CheckBox<Msg>,
    autoplay: CheckBox<Msg>,
}

impl SessionSection {
    /// Builds the section's heading and two checkboxes.
    pub(super) fn new(form: &FormPage) -> SessionSection {
        let heading = make(form.ui(), label("Playback")).expect("create playback heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let resume = make(form.ui(), checkbox("Resume playback on startup"))
            .expect("create resume checkbox")
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::ResumePlayback(on))));
        form.add_full(resume.id(), ROW_HEIGHT);

        let autoplay = make(form.ui(), checkbox("Start playing when resuming"))
            .expect("create autoplay checkbox")
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::AutoplayOnRestore(on))));
        form.add_full(autoplay.id(), ROW_HEIGHT);

        SessionSection {
            _heading: heading,
            resume,
            autoplay,
        }
    }

    /// Mirrors the shared state onto the checkboxes.
    pub(super) fn sync(&self, state: &AppState) {
        self.resume.set_checked(state.resume_playback);
        self.autoplay.set_checked(state.autoplay_on_restore);
        self.autoplay.set_enabled(state.resume_playback);
    }

    /// Handles the session-resume messages; returns whether `msg` was one.
    pub(super) fn update(&self, msg: &SettingsMsg, state: &mut AppState) -> bool {
        match msg {
            SettingsMsg::ResumePlayback(on) => state.resume_playback = *on,
            SettingsMsg::AutoplayOnRestore(on) => state.autoplay_on_restore = *on,
            _ => return false,
        }
        true
    }
}
