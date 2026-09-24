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
use win32ui::prelude::*;
use win32ui::{Button, Edit};

use crate::app::Msg;

use super::super::{HEADING_HEIGHT, ROW_HEIGHT, SettingsMsg, labelled};
use super::{parse, path_text};

/// Width of the path field, in design units.
const FIELD_WIDTH: f32 = 320.0;

/// The SID song-length controls.
pub(super) struct SidSection {
    heading: Label,
    database_label: Label,
    edit: Edit<Msg>,
    browse_file: Button<Msg>,
    browse_folder: Button<Msg>,
    clear: Button<Msg>,
    status: Label,
    fallback_label: Label,
    fallback: Slider<Msg>,
    hint: Label,
    proxy: win32ui::Proxy<Msg>,
    /// The path last mirrored into the text field, so typing is not clobbered.
    applied_path: Option<PathBuf>,
}

impl SidSection {
    /// Builds the section's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>, proxy: win32ui::Proxy<Msg>) -> win32ui::Result<Self> {
        let edit = Edit::single_line(ui)?
            .cue("Songlengths.md5, or an HVSC root folder")
            .on_submit(|| Some(Msg::Settings(SettingsMsg::SidCommit)))
            .on_focus(|focused| (!focused).then_some(Msg::Settings(SettingsMsg::SidCommit)));
        let browse_file = Button::new(ui, "Browse file...")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::SidBrowseFile)));
        let browse_folder = Button::new(ui, "Browse folder...")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::SidBrowseFolder)));
        let clear =
            Button::new(ui, "Clear")?.on_click(|| Some(Msg::Settings(SettingsMsg::SidClear)));
        let fallback = Slider::new(ui, 30.0..=600.0)?.on_change(|value| {
            Some(Msg::Settings(
                SettingsMsg::SidFallback(value.round() as u32),
            ))
        });

        Ok(Self {
            heading: Label::new(ui, Rect::default(), "SID song lengths")?,
            database_label: Label::new(ui, Rect::default(), "HVSC Songlengths")?,
            edit,
            browse_file,
            browse_folder,
            clear,
            status: Label::new(ui, Rect::default(), "")?,
            fallback_label: Label::new(ui, Rect::default(), "Fallback length")?,
            fallback,
            hint: Label::new(
                ui,
                Rect::default(),
                "SID tunes with no database entry play for this long, without a shown total, so the queue still advances.",
            )?,
            proxy,
            applied_path: None,
        })
    }

    /// The section's controls as layout items, in display order.
    pub(super) fn items(&self) -> Vec<LayoutItem> {
        let mut field = Layout::row().spacing(dip(8.0));
        field = field
            .item(self.edit.width(dip(FIELD_WIDTH)))
            .item(&self.browse_file)
            .item(&self.browse_folder)
            .item(&self.clear);
        vec![
            self.heading.height(dip(HEADING_HEIGHT)),
            labelled(&self.database_label, field.height(dip(ROW_HEIGHT))),
            self.status.height(dip(ROW_HEIGHT)),
            labelled(&self.fallback_label, self.fallback.fill(1)),
            self.hint.height(dip(ROW_HEIGHT)),
        ]
    }

    /// Shows or hides every control on the section.
    pub(super) fn set_visible(&self, visible: bool) {
        self.heading.set_visible(visible);
        self.database_label.set_visible(visible);
        self.edit.set_visible(visible);
        self.browse_file.set_visible(visible);
        self.browse_folder.set_visible(visible);
        self.clear.set_visible(visible);
        self.status.set_visible(visible);
        self.fallback_label.set_visible(visible);
        self.fallback.set_visible(visible);
        self.hint.set_visible(visible);
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
