//! Settings → Visualization page (#306): projectM timing/audio settings, the
//! preset packs and their on-disk status, the optional user preset folder and
//! the projectM engine status.
//!
//! Every field change is pushed as [`Command::Viz`]`(`[`VizCommand::SetSettings`]`)`,
//! which the shell applies to the shared state and the running surface; the
//! pack list is counted on a background thread so thousands of `.milk` files
//! never stall the UI. `Panel`/`Window`/`Fullscreen` placement and the preset
//! actions live in the View menu and the surface's context menu, not here.

mod form;
mod packs;
mod packs_section;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use emusic_ui::state::projectm::ProjectMAvailability;
use emusic_ui::state::{AppState, Command, VizCommand};
use emusic_ui::views::Commands;
use xui::prelude::*;

use crate::app::Msg;

use super::{HEADING_HEIGHT, ROW_HEIGHT, ScrollPanel, SettingsMsg};
use packs_section::PacksSection;

pub use form::VisualizationEdit;

/// The Visualization page's controls.
pub(super) struct VisualizationPage {
    form: ScrollPanel,
    heading: Label,
    status: Label,
    timing: form::Form,
    packs: PacksSection,
    proxy: xui::Proxy<Msg>,
    exe_dir: PathBuf,
    helper: PathBuf,
    /// The pack preset counts, once the background scan finished.
    counts: RefCell<Option<Vec<(String, usize)>>>,
    /// Whether the background scan has been started.
    scan_started: Cell<bool>,
}

impl VisualizationPage {
    /// Builds the page's controls. The preset-pack count starts only on the
    /// first [`Self::sync`], so launching the app never walks the packs.
    pub(super) fn new(ui: &mut Ui<Msg>) -> xui::Result<Self> {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(PathBuf::from))
            .unwrap_or_default();
        let form = ScrollPanel::new(ui)?;
        let mut panel = form.ui(ui);
        let timing = form::Form::new(&mut panel)?;
        let packs = PacksSection::new(&mut panel)?;
        let page = Self {
            form,
            heading: Label::new(&mut panel, Rect::default(), "Visualization")?,
            status: Label::new(&mut panel, Rect::default(), "")?,
            timing,
            packs,
            proxy: ui.proxy(),
            helper: exe_dir.join("emusic-presets-setup.exe"),
            exe_dir,
            counts: RefCell::new(None),
            scan_started: Cell::new(false),
        };
        page.apply(ui);
        Ok(page)
    }

    /// The page's scrollable form as one tab-strip page.
    pub(super) fn page(&self) -> LayoutItem {
        self.form.page()
    }

    /// The page's controls as form rows, in display order.
    fn rows(&self) -> Vec<super::FormRow> {
        let mut rows = vec![
            (self.heading.height(dip(HEADING_HEIGHT)), HEADING_HEIGHT),
            (self.status.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
        ];
        rows.extend(self.timing.rows());
        rows.extend(self.packs.rows());
        rows.extend(self.timing.user_rows());
        rows
    }

    /// Reinstalls the page's form (used after the sensitivity row appears or
    /// hides).
    fn apply(&self, ui: &Ui<Msg>) {
        self.form.apply(ui, self.rows());
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.form.set_visible(visible);
    }

    /// Pushes the shared state onto the controls and starts the pack count on
    /// the first call.
    pub(super) fn sync(&mut self, ui: &Ui<Msg>, state: &AppState) {
        self.status
            .set_text(&status_text(&state.projectm.availability));
        let reapply = self.timing.sync(&state.projectm.settings);
        self.ensure_scan();
        self.packs
            .sync(state, &self.exe_dir, self.counts.borrow().as_deref());
        self.packs.sync_helper(self.helper.is_file(), &self.exe_dir);
        if reapply {
            self.apply(ui);
        }
    }

    /// Starts the background pack scan once per session.
    fn ensure_scan(&self) {
        if self.scan_started.replace(true) {
            return;
        }
        let proxy = self.proxy.clone();
        let exe_dir = self.exe_dir.clone();
        let spawned = std::thread::Builder::new()
            .name("emusic-viz-pack-count".to_owned())
            .spawn(move || {
                let counts = packs::count_installed(&exe_dir);
                let _ = proxy.send(Msg::Settings(SettingsMsg::VizPackCounts(counts)));
            });
        if spawned.is_err() {
            self.scan_started.set(false);
        }
    }

    /// Handles the Visualization page's messages; returns whether `msg` was
    /// one.
    pub(super) fn update(
        &mut self,
        msg: &SettingsMsg,
        ui: &Ui<Msg>,
        state: &mut AppState,
        out: &mut Commands,
    ) -> bool {
        match msg {
            SettingsMsg::Viz(edit) => {
                let mut settings = state.projectm.settings.clone();
                form::apply_edit(edit, &mut settings);
                if let VisualizationEdit::HardCuts(on) = edit {
                    self.timing.set_hard_cuts(*on);
                    self.apply(ui);
                }
                out.push(Command::Viz(VizCommand::SetSettings(settings)));
            }
            SettingsMsg::VizPack(pack, on) => {
                let mut settings = state.projectm.settings.clone();
                settings.disabled_packs.retain(|disabled| disabled != pack);
                if !*on {
                    settings.disabled_packs.push(pack.clone());
                }
                out.push(Command::Viz(VizCommand::SetSettings(settings)));
            }
            SettingsMsg::VizGetPresets => {
                if let Err(error) = std::process::Command::new(&self.helper).spawn() {
                    tracing::warn!(
                        %error,
                        helper = %self.helper.display(),
                        "could not launch the preset setup helper"
                    );
                }
            }
            SettingsMsg::VizBrowse => {
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let picked = rfd::FileDialog::new().pick_folder();
                    let _ = proxy.send(Msg::Settings(SettingsMsg::VizPicked(picked)));
                });
            }
            SettingsMsg::VizPicked(Some(path)) => {
                self.timing.set_user_dir_text(Some(path));
                let mut settings = state.projectm.settings.clone();
                settings.user_preset_dir = Some(path.clone());
                out.push(Command::Viz(VizCommand::SetSettings(settings)));
            }
            SettingsMsg::VizPicked(None) => {}
            SettingsMsg::VizCommitUserDir => {
                let mut settings = state.projectm.settings.clone();
                settings.user_preset_dir = self.timing.user_dir_text();
                out.push(Command::Viz(VizCommand::SetSettings(settings)));
            }
            SettingsMsg::VizClearUserDir => {
                self.timing.set_user_dir_text(None);
                let mut settings = state.projectm.settings.clone();
                settings.user_preset_dir = None;
                out.push(Command::Viz(VizCommand::SetSettings(settings)));
            }
            SettingsMsg::VizPackCounts(counts) => {
                *self.counts.borrow_mut() = Some(counts.clone());
            }
            _ => return false,
        }
        true
    }
}

/// The projectM engine status line (#306).
fn status_text(availability: &ProjectMAvailability) -> String {
    match availability {
        ProjectMAvailability::Unknown => "starting...".to_owned(),
        ProjectMAvailability::Available(version) if version.is_empty() => {
            "projectM running".to_owned()
        }
        ProjectMAvailability::Available(version) if version.starts_with("projectM") => {
            version.clone()
        }
        ProjectMAvailability::Available(version) => format!("projectM {version}"),
        ProjectMAvailability::MissingLibrary => "projectM not installed".to_owned(),
        ProjectMAvailability::NoOpenGl => "OpenGL 3.3 unavailable".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_is_worded_for_the_settings_page() {
        assert_eq!(status_text(&ProjectMAvailability::Unknown), "starting...");
        assert_eq!(
            status_text(&ProjectMAvailability::MissingLibrary),
            "projectM not installed"
        );
        assert_eq!(
            status_text(&ProjectMAvailability::NoOpenGl),
            "OpenGL 3.3 unavailable"
        );
        assert_eq!(
            status_text(&ProjectMAvailability::Available("4.1.7".to_owned())),
            "projectM 4.1.7"
        );
        assert_eq!(
            status_text(&ProjectMAvailability::Available(
                "projectM 4.1.7".to_owned()
            )),
            "projectM 4.1.7"
        );
    }
}
