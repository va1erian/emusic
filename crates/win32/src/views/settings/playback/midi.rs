//! Playback page → MIDI soundfont: the `.sf2`/`.sf3`/`.sfz` file BASSMIDI
//! renders MIDI files with.
//!
//! BASSMIDI is a software synthesizer, so without a soundfont MIDI files play
//! silently. The path is edited as text or picked with a native file dialog
//! (run on its own thread so the UI never blocks); it is pushed as
//! [`Command::SetMidiSoundfont`] and checked inline for the common mistakes.

use std::path::{Path, PathBuf};

use emusic_player::midi::soundfont_problem;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Commands;
use win32ui::prelude::*;
use win32ui::{Button, ComboBox, Edit, Proxy};

use crate::app::Msg;

use super::super::{FormRow, HEADING_HEIGHT, ROW_HEIGHT, SettingsMsg, labelled};
use super::{parse, path_text};

/// Width of the path field, in design units.
const FIELD_WIDTH: f32 = 360.0;

/// The MIDI soundfont controls.
pub(super) struct MidiSection {
    heading: Label,
    soundfont_label: Label,
    edit: Edit<Msg>,
    browse: Button<Msg>,
    clear: Button<Msg>,
    status: Label,
    recent_label: Label,
    recent: ComboBox<PathBuf, Msg>,
    proxy: Proxy<Msg>,
    /// The path last mirrored into the text field, so typing is not clobbered.
    applied_path: Option<PathBuf>,
    /// The recent list last mirrored into the combo box.
    applied_recent: Vec<PathBuf>,
}

impl MidiSection {
    /// Builds the section's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>, proxy: Proxy<Msg>) -> win32ui::Result<Self> {
        let edit = Edit::single_line(ui)?
            .cue("path to a .sf2 / .sf3 / .sfz file")
            .on_submit(|| Some(Msg::Settings(SettingsMsg::MidiCommit)))
            .on_focus(|focused| (!focused).then_some(Msg::Settings(SettingsMsg::MidiCommit)));
        let browse =
            Button::new(ui, "Browse...")?.on_click(|| Some(Msg::Settings(SettingsMsg::MidiBrowse)));
        let clear =
            Button::new(ui, "Clear")?.on_click(|| Some(Msg::Settings(SettingsMsg::MidiClear)));
        let recent = ComboBox::new(ui, Vec::<(String, PathBuf)>::new())?
            .on_select(|path| Some(Msg::Settings(SettingsMsg::MidiPicked(Some(path.clone())))));

        Ok(Self {
            heading: Label::new(ui, Rect::default(), "MIDI playback")?,
            soundfont_label: Label::new(ui, Rect::default(), "Soundfont")?,
            edit,
            browse,
            clear,
            status: Label::new(ui, Rect::default(), "")?,
            recent_label: Label::new(ui, Rect::default(), "Recent soundfonts")?,
            recent,
            proxy,
            applied_path: None,
            applied_recent: Vec::new(),
        })
    }

    /// The section's controls as form rows, in display order.
    pub(super) fn rows(&self) -> Vec<FormRow> {
        let mut field = Layout::row().spacing(dip(8.0));
        field = field
            .item(self.edit.width(dip(FIELD_WIDTH)))
            .item(&self.browse)
            .item(&self.clear);
        vec![
            (self.heading.height(dip(HEADING_HEIGHT)), HEADING_HEIGHT),
            (
                labelled(&self.soundfont_label, field.height(dip(ROW_HEIGHT))),
                ROW_HEIGHT,
            ),
            (self.status.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (self.recent_label.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (
                row![self.recent.width(dip(FIELD_WIDTH))].height(dip(ROW_HEIGHT)),
                ROW_HEIGHT,
            ),
        ]
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

        if self.applied_recent != state.recent_soundfonts {
            self.applied_recent = state.recent_soundfonts.clone();
            self.recent.set_items(
                state
                    .recent_soundfonts
                    .iter()
                    .map(|path| (path.display().to_string(), path.clone())),
            );
        }
    }

    /// Handles the MIDI messages; returns whether `msg` was one.
    pub(super) fn update(&self, msg: &SettingsMsg, out: &mut Commands) -> bool {
        match msg {
            SettingsMsg::MidiBrowse => {
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let picked = rfd::FileDialog::new()
                        .add_filter("Soundfont", &["sf2", "sf3", "sfz"])
                        .pick_file();
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
            "No soundfont set: MIDI files play silently unless one sits next to \
             the BASS DLLs."
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

    #[test]
    fn parse_trims_whitespace_and_quotes() {
        assert_eq!(
            parse(r#"  "C:\fonts\gm.sf2"  "#),
            Some(PathBuf::from(r"C:\fonts\gm.sf2"))
        );
        assert_eq!(parse("   "), None);
    }

    #[test]
    fn unset_and_missing_soundfonts_warn() {
        let (message, problem) = status(None);
        assert!(problem && message.contains("silently"));
        let (message, problem) = status(Some(Path::new("Z:/definitely/absent.sf2")));
        assert!(problem && message.contains("File not found"));
    }
}
