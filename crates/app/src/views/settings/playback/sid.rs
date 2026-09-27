//! Playback page → SID song lengths (#192): where the HVSC `Songlengths.md5`
//! database lives, and the fallback play length for tunes it does not cover.
//!
//! SID files carry no length, so without a database the player cannot show a
//! real total (it shows elapsed time only and stops after the fallback length,
//! so the queue still advances). The path may be either the `Songlengths.md5`
//! file or an HVSC root folder, in which the database is auto-detected.

use std::path::{Path, PathBuf};

use emusic_player::sid::resolve_database_path;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Commands;
use xui::xui_core::app::Proxy;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, Edit, HasText, Label, Slider};

use super::super::SettingsMsg;
use super::super::form::{FormPage, HEADING_HEIGHT, LABEL_WIDTH, ROW_HEIGHT};
use super::{parse, path_text};
use crate::app::Msg;

/// Width of each SID button, in design units.
const BUTTON_WIDTH: f32 = 110.0;
/// Height of the explanatory hint, in design units.
const HINT_HEIGHT: f32 = 40.0;

/// The SID song-length controls.
pub(super) struct SidSection {
    _heading: Label<Msg>,
    _database_label: Label<Msg>,
    edit: Edit<Msg>,
    _browse_file: Button<Msg>,
    _browse_folder: Button<Msg>,
    _clear: Button<Msg>,
    _apply: Button<Msg>,
    status: Label<Msg>,
    _fallback_label: Label<Msg>,
    fallback: Slider<Msg>,
    _hint: Label<Msg>,
    proxy: Proxy<Msg>,
    /// The path last mirrored into the text field, so typing is not clobbered.
    applied_path: Option<PathBuf>,
}

impl SidSection {
    /// Builds the section's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(form: &FormPage, proxy: Proxy<Msg>) -> SidSection {
        let heading =
            Label::new(form.ui(), Rect::default(), "SID song lengths").expect("create sid heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let row = form.row(ROW_HEIGHT).expect("create sid row");
        let database_label =
            Label::new(row.ui(), Rect::default(), "HVSC Songlengths").expect("create sid label");
        let edit = Edit::new(row.ui(), Rect::default(), "").expect("create sid field");
        let browse_file = Button::new(row.ui(), Rect::default(), "Browse file...")
            .expect("create sid file button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::SidBrowseFile)));
        let browse_folder = Button::new(row.ui(), Rect::default(), "Browse folder...")
            .expect("create sid folder button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::SidBrowseFolder)));
        let clear = Button::new(row.ui(), Rect::default(), "Clear")
            .expect("create sid clear")
            .on_click(|| Some(Msg::Settings(SettingsMsg::SidClear)));
        let apply = Button::new(row.ui(), Rect::default(), "Apply")
            .expect("create sid apply")
            .on_click(|| Some(Msg::Settings(SettingsMsg::SidCommit)));
        row.fixed(database_label.id(), LABEL_WIDTH)
            .fill(edit.id())
            .fixed(browse_file.id(), dip(BUTTON_WIDTH))
            .fixed(browse_folder.id(), dip(BUTTON_WIDTH))
            .fixed(clear.id(), dip(BUTTON_WIDTH))
            .fixed(apply.id(), dip(BUTTON_WIDTH))
            .finish();

        let status = Label::new(form.ui(), Rect::default(), "").expect("create sid status");
        form.add_full(status.id(), ROW_HEIGHT);

        let fallback_row = form.row(ROW_HEIGHT).expect("create fallback row");
        let fallback_label = Label::new(fallback_row.ui(), Rect::default(), "Fallback length")
            .expect("create fallback label");
        let fallback = Slider::new(fallback_row.ui(), Rect::default(), 30.0, 600.0)
            .expect("create fallback slider")
            .on_change(|value| {
                Some(Msg::Settings(
                    SettingsMsg::SidFallback(value.round() as u32),
                ))
            });
        fallback_row
            .fixed(fallback_label.id(), LABEL_WIDTH)
            .fill(fallback.id())
            .finish();

        let hint = Label::new(
            form.ui(),
            Rect::default(),
            "SID tunes with no database entry play for this long, without a shown total, so the \
             queue still advances.",
        )
        .expect("create sid hint");
        form.add_full(hint.id(), dip(HINT_HEIGHT));

        SidSection {
            _heading: heading,
            _database_label: database_label,
            edit,
            _browse_file: browse_file,
            _browse_folder: browse_folder,
            _clear: clear,
            _apply: apply,
            status,
            _fallback_label: fallback_label,
            fallback,
            _hint: hint,
            proxy,
            applied_path: None,
        }
    }

    /// Mirrors the shared state onto the controls.
    pub(super) fn sync(&mut self, state: &AppState) {
        if self.applied_path != state.songlengths_path {
            self.applied_path = state.songlengths_path.clone();
            self.edit
                .set_text(&path_text(state.songlengths_path.as_deref()));
        }
        let (message, problem) = status(state.songlengths_path.as_deref());
        self.status.set_text(&if problem {
            format!("Warning: {message}")
        } else {
            message
        });
        self.fallback.set_value(f64::from(state.sid_fallback_secs));
    }

    /// Handles the SID messages; returns whether `msg` was one.
    pub(super) fn update(&self, msg: &SettingsMsg, out: &mut Commands) -> bool {
        match msg {
            SettingsMsg::SidBrowseFile => {
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let picked = rfd::FileDialog::new()
                        .add_filter("Songlengths", &["md5", "txt"])
                        .pick_file();
                    let _ = proxy.send(Msg::Settings(SettingsMsg::SidPicked(picked)));
                });
            }
            SettingsMsg::SidBrowseFolder => {
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let picked = rfd::FileDialog::new().pick_folder();
                    let _ = proxy.send(Msg::Settings(SettingsMsg::SidPicked(picked)));
                });
            }
            SettingsMsg::SidPicked(Some(path)) => {
                self.edit.set_text(&path.display().to_string());
                out.push(Command::SetSonglengthsPath(Some(path.clone())));
            }
            SettingsMsg::SidPicked(None) => {}
            SettingsMsg::SidCommit => {
                out.push(Command::SetSonglengthsPath(parse(&self.edit.text())));
            }
            SettingsMsg::SidClear => {
                self.edit.set_text("");
                out.push(Command::SetSonglengthsPath(None));
            }
            SettingsMsg::SidFallback(secs) => out.push(Command::SetSidFallbackSecs(*secs)),
            _ => return false,
        }
        true
    }
}

/// The inline message under the field and whether it is a warning.
fn status(path: Option<&Path>) -> (String, bool) {
    match path {
        None => (
            "No database: SID tunes show elapsed time only and stop after the fallback length."
                .to_string(),
            true,
        ),
        Some(path) => match resolve_database_path(path) {
            Some(database) => (format!("Using {}", database.display()), false),
            None => (
                format!("No Songlengths.md5 found at {}", path.display()),
                true,
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_and_missing_paths_warn() {
        let (message, problem) = status(None);
        assert!(problem && message.contains("elapsed time only"));
        let (message, problem) = status(Some(Path::new("Z:/definitely/absent")));
        assert!(problem && message.contains("No Songlengths.md5"));
    }
}
