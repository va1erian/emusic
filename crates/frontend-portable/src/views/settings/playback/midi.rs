//! Playback page → MIDI soundfont: the `.sf2`/`.sf3`/`.sfz` file BASSMIDI
//! renders MIDI files with.
//!
//! BASSMIDI is a software synthesizer, so without a soundfont MIDI files play
//! silently. The path is edited as text — committed with Apply, or picked with
//! a file dialog run on its own thread so the UI never blocks — and pushed as
//! [`Command::SetMidiSoundfont`](emusic_ui::state::Command::SetMidiSoundfont).

use std::path::Path;

use emusic_player::midi::soundfont_problem;
use emusic_ui::file_picker::{self, Filter};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Commands;
use xui::xui_core::app::Proxy;
use xui::xui_core::widget::{Button, Edit, HasText, Label};

use super::super::SettingsMsg;
use super::super::form::{FormPage, HEADING_HEIGHT, LABEL_WIDTH, ROW_HEIGHT};
use super::{parse, path_text};
use crate::app::Msg;
use crate::make::make;
use xui::xui_core::arrange::{button, edit, label};

/// Width of the browse/clear/apply buttons, in design units.
const BUTTON_WIDTH: f32 = 90.0;

/// The MIDI soundfont controls.
pub(super) struct MidiSection {
    _heading: Label<Msg>,
    _soundfont_label: Label<Msg>,
    edit: Edit<Msg>,
    _browse: Button<Msg>,
    _clear: Button<Msg>,
    _apply: Button<Msg>,
    status: Label<Msg>,
    proxy: Proxy<Msg>,
    /// The path last mirrored into the text field, so typing is not clobbered.
    applied_path: Option<std::path::PathBuf>,
}

impl MidiSection {
    /// Builds the section's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(form: &FormPage, proxy: Proxy<Msg>) -> MidiSection {
        let heading = make(form.ui(), label("MIDI playback")).expect("create midi heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let row = form.row(ROW_HEIGHT).expect("create soundfont row");
        let soundfont_label = make(row.ui(), label("Soundfont")).expect("create soundfont label");
        let edit = make(row.ui(), edit()).expect("create soundfont field");
        let browse = make(row.ui(), button("Browse..."))
            .expect("create soundfont browse")
            .on_click(|| Some(Msg::Settings(SettingsMsg::MidiBrowse)));
        let clear = make(row.ui(), button("Clear"))
            .expect("create soundfont clear")
            .on_click(|| Some(Msg::Settings(SettingsMsg::MidiClear)));
        let apply = make(row.ui(), button("Apply"))
            .expect("create soundfont apply")
            .on_click(|| Some(Msg::Settings(SettingsMsg::MidiCommit)));
        row.fixed(soundfont_label.id(), LABEL_WIDTH)
            .fill(edit.id())
            .fixed(browse.id(), xui::xui_core::units::dip(BUTTON_WIDTH))
            .fixed(clear.id(), xui::xui_core::units::dip(BUTTON_WIDTH))
            .fixed(apply.id(), xui::xui_core::units::dip(BUTTON_WIDTH))
            .finish();

        let status = make(form.ui(), label("")).expect("create soundfont status");
        form.add_full(status.id(), ROW_HEIGHT);

        MidiSection {
            _heading: heading,
            _soundfont_label: soundfont_label,
            edit,
            _browse: browse,
            _clear: clear,
            _apply: apply,
            status,
            proxy,
            applied_path: None,
        }
    }

    /// Mirrors the shared state onto the controls.
    pub(super) fn sync(&mut self, state: &AppState) {
        if self.applied_path != state.midi_soundfont {
            self.applied_path = state.midi_soundfont.clone();
            self.edit
                .set_text(&path_text(state.midi_soundfont.as_deref()));
        }
        let (message, problem) = status(state.midi_soundfont.as_deref());
        self.status.set_text(&if problem {
            format!("Warning: {message}")
        } else {
            message
        });
    }

    /// Handles the MIDI messages; returns whether `msg` was one.
    pub(super) fn update(&self, msg: &SettingsMsg, out: &mut Commands) -> bool {
        match msg {
            SettingsMsg::MidiBrowse => {
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let picked = file_picker::pick_file(Filter {
                        name: "Soundfont",
                        extensions: &["sf2", "sf3", "sfz"],
                    });
                    let _ = proxy.send(Msg::Settings(SettingsMsg::MidiPicked(picked)));
                });
            }
            SettingsMsg::MidiPicked(Some(path)) => {
                self.edit.set_text(&path.display().to_string());
                out.push(Command::SetMidiSoundfont(Some(path.clone())));
            }
            SettingsMsg::MidiPicked(None) => {}
            SettingsMsg::MidiCommit => {
                out.push(Command::SetMidiSoundfont(parse(&self.edit.text())));
            }
            SettingsMsg::MidiClear => {
                self.edit.set_text("");
                out.push(Command::SetMidiSoundfont(None));
            }
            _ => return false,
        }
        true
    }
}

/// The inline message under the field and whether it is a warning.
fn status(path: Option<&Path>) -> (String, bool) {
    match path {
        None => (
            "No soundfont set: MIDI files play silently unless one sits next to the BASS DLLs."
                .to_string(),
            true,
        ),
        Some(path) => match soundfont_problem(path) {
            Some(problem) => (
                format!(
                    "{problem} MIDI files play silently unless a soundfont sits next to the BASS DLLs."
                ),
                true,
            ),
            None => (format!("Using {}", path.display()), false),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn unset_and_missing_soundfonts_warn() {
        let (message, problem) = status(None);
        assert!(problem && message.contains("silently"));
        let (message, problem) = status(Some(Path::new("Z:/definitely/absent.sf2")));
        assert!(problem && message.contains("File not found"));
    }

    #[test]
    fn parse_trims_whitespace_and_quotes() {
        assert_eq!(
            parse(r#"  "C:\fonts\gm.sf2"  "#),
            Some(PathBuf::from(r"C:\fonts\gm.sf2"))
        );
        assert_eq!(parse("   "), None);
    }
}
