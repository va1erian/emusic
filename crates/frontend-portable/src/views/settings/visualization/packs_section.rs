//! The Visualization page's preset-pack section (#306): one enable checkbox
//! and an installed/count status per pack, plus the "Get more presets..."
//! helper launcher.

use std::path::Path;

use emusic_ui::state::AppState;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, CheckBox, HasText, Label};

use super::super::SettingsMsg;
use super::super::form::{FormPage, HEADING_HEIGHT, ROW_HEIGHT};
use super::packs::{self, KNOWN_PACKS};
use crate::app::Msg;

/// Width of a pack's enable checkbox, in design units.
const PACK_NAME_WIDTH: f32 = 220.0;
/// Width of a pack's status label, in design units.
const PACK_STATUS_WIDTH: f32 = 200.0;

/// One preset pack's row: its enable checkbox and its status text.
struct PackRow {
    pack: &'static str,
    checkbox: CheckBox<Msg>,
    status: Label<Msg>,
}

/// The preset-pack section.
pub(super) struct PacksSection {
    _heading: Label<Msg>,
    packs: Vec<PackRow>,
    get_more: Button<Msg>,
    note: Label<Msg>,
}

impl PacksSection {
    /// Builds the section's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(form: &FormPage) -> PacksSection {
        let heading =
            Label::new(form.ui(), Rect::default(), "Preset packs").expect("create packs heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let mut packs = Vec::with_capacity(KNOWN_PACKS.len());
        for pack in KNOWN_PACKS {
            let row = form.row(ROW_HEIGHT).expect("create pack row");
            let checkbox = CheckBox::new(row.ui(), Rect::default(), pack)
                .expect("create pack checkbox")
                .on_toggle(move |on| {
                    Some(Msg::Settings(SettingsMsg::VizPack(pack.to_owned(), on)))
                });
            let status = Label::new(row.ui(), Rect::default(), "").expect("create pack status");
            row.fixed(checkbox.id(), dip(PACK_NAME_WIDTH))
                .fixed(status.id(), dip(PACK_STATUS_WIDTH))
                .finish();
            packs.push(PackRow {
                pack,
                checkbox,
                status,
            });
        }

        let get_more = Button::new(form.ui(), Rect::default(), "Get more presets...")
            .expect("create get-more button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::VizGetPresets)));
        form.add_full(get_more.id(), ROW_HEIGHT);

        let note = Label::new(form.ui(), Rect::default(), "").expect("create packs note");
        form.add_full(note.id(), ROW_HEIGHT);

        PacksSection {
            _heading: heading,
            packs,
            get_more,
            note,
        }
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
