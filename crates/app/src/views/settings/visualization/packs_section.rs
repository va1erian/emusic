#![forbid(unsafe_code)]

//! The Visualization page's preset-pack section (#306): one enable checkbox
//! and an installed/count status per pack, plus the "Get more presets..."
//! helper launcher.

use std::path::Path;

use emusic_ui::state::AppState;
use xui::prelude::*;
use xui::{Button, CheckBox};

use crate::app::Msg;

use super::super::{FormRow, HEADING_HEIGHT, ROW_HEIGHT, SettingsMsg};
use super::packs::{self, KNOWN_PACKS};

/// Width of a pack's enable checkbox, in design units.
const PACK_NAME_WIDTH: f32 = 220.0;
/// Width of a pack's status label, in design units.
const PACK_STATUS_WIDTH: f32 = 200.0;
/// Tooltip for a pack's enable checkbox.
const TIP_PACK: &str = "Include this pack's presets in the rotation.";
/// Tooltip for the "Get more presets..." button.
const TIP_GET_MORE: &str = "Download the optional preset packs listed here.";

/// One preset pack's row: its enable checkbox and its status text.
struct PackRow {
    pack: &'static str,
    checkbox: CheckBox<Msg>,
    status: Label,
}

/// The preset-pack section.
pub(super) struct PacksSection {
    heading: Label,
    packs: Vec<PackRow>,
    get_more: Button<Msg>,
    note: Label,
}

impl PacksSection {
    /// Builds the section's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>) -> xui::Result<Self> {
        let mut packs = Vec::with_capacity(KNOWN_PACKS.len());
        for pack in KNOWN_PACKS {
            let checkbox = CheckBox::new(ui, pack)?.on_toggle(move |on| {
                Some(Msg::Settings(SettingsMsg::VizPack(pack.to_owned(), on)))
            });
            checkbox.set_tooltip(TIP_PACK);
            packs.push(PackRow {
                pack,
                checkbox,
                status: Label::new(ui, Rect::default(), "")?,
            });
        }
        let get_more = Button::new(ui, "Get more presets...")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::VizGetPresets)));
        get_more.set_tooltip(TIP_GET_MORE);
        Ok(Self {
            heading: Label::new(ui, Rect::default(), "Preset packs")?,
            packs,
            get_more,
            note: Label::new(ui, Rect::default(), "")?,
        })
    }

    /// The section's rows, in display order.
    pub(super) fn rows(&self) -> Vec<FormRow> {
        let mut rows = vec![(self.heading.height(dip(HEADING_HEIGHT)), HEADING_HEIGHT)];
        for pack in &self.packs {
            rows.push((
                Layout::row()
                    .spacing(dip(16.0))
                    .item(pack.checkbox.width(dip(PACK_NAME_WIDTH)))
                    .item(pack.status.width(dip(PACK_STATUS_WIDTH)))
                    .height(dip(ROW_HEIGHT)),
                ROW_HEIGHT,
            ));
        }
        rows.push((self.get_more.height(dip(ROW_HEIGHT)), ROW_HEIGHT));
        rows.push((self.note.height(dip(ROW_HEIGHT)), ROW_HEIGHT));
        rows
    }

    /// Mirrors the pack enable flags and their installed/count status.
    pub(super) fn sync(
        &self,
        state: &AppState,
        exe_dir: &Path,
        counts: Option<&[(String, usize)]>,
    ) {
        for row in &self.packs {
            row.checkbox
                .set_checked(state.projectm.settings.pack_enabled(row.pack));
            let installed = packs::is_installed(exe_dir, row.pack);
            let count = counts
                .and_then(|counts| counts.iter().find(|(pack, _)| pack == row.pack))
                .map(|(_, count)| *count);
            row.status.set_text(&pack_status(installed, count));
        }
    }

    /// Enables the button only while its helper sits next to the executable,
    /// and explains why when it does not.
    pub(super) fn sync_helper(&self, present: bool, exe_dir: &Path) {
        self.get_more.set_enabled(present);
        self.note.set_text(&if present {
            "Opens the optional preset downloader (cream-of-the-crop, en-d, projectm-classic)."
                .to_owned()
        } else {
            format!(
                "emusic-presets-setup.exe was not found next to {}.",
                exe_dir.display()
            )
        });
    }
}

/// A pack's status line: installed with a preset count, or not installed.
fn pack_status(installed: bool, count: Option<usize>) -> String {
    if !installed {
        "not installed".to_owned()
    } else {
        match count {
            Some(count) => format!("{count} presets"),
            None => "counting...".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_status_shows_installed_counts_and_missing_packs() {
        assert_eq!(pack_status(false, None), "not installed");
        assert_eq!(pack_status(true, None), "counting...");
        assert_eq!(pack_status(true, Some(552)), "552 presets");
    }
}
