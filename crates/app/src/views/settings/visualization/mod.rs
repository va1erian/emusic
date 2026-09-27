//! Settings → Visualization page (#306): projectM timing/audio settings, the
//! preset packs and their on-disk status, the optional user preset folder and
//! the projectM engine status.
//!
//! Every field change is pushed as [`Command::Viz`] with a
//! [`VizCommand::SetSettings`] payload; the shell applies it to the shared
//! state and the running surface. The pack list is counted on a background
//! thread so thousands of `.milk` files never stall the UI.
//!
//! [`Command::Viz`]: emusic_ui::state::Command::Viz

mod form;
mod packs;
mod packs_section;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use emusic_ui::state::projectm::ProjectMAvailability;
use emusic_ui::state::{AppState, Command, VizCommand};
use emusic_ui::views::Commands;
use xui::xui_core::app::{Proxy, Ui};
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::{HasText, Label};

use super::SettingsMsg;
use super::form::{FormPage, HEADING_HEIGHT, ROW_HEIGHT};
use crate::app::Msg;

pub use form::VisualizationEdit;
use packs_section::PacksSection;

/// The Visualization page's controls.
pub(super) struct VisualizationPage {
    form: FormPage,
    _heading: Label<Msg>,
    status: Label<Msg>,
    timing: form::TimingForm,
    packs: PacksSection,
    proxy: Proxy<Msg>,
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
    pub(super) fn new(ui: &Ui<Msg>) -> VisualizationPage {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(PathBuf::from))
            .unwrap_or_default();
        let form = FormPage::new(ui).expect("create visualization form");

        let heading = Label::new(form.ui(), Rect::default(), "Visualization")
            .expect("create visualization heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let status =
            Label::new(form.ui(), Rect::default(), "").expect("create visualization status");
        form.add_full(status.id(), ROW_HEIGHT);

        let timing = form::TimingForm::new(&form);
        let packs = PacksSection::new(&form);
        VisualizationPage {
            form,
            _heading: heading,
            status,
            timing,
            packs,
            proxy: ui.proxy(),
            helper: exe_dir.join("emusic-presets-setup.exe"),
            exe_dir,
            counts: RefCell::new(None),
            scan_started: Cell::new(false),
        }
    }

    /// The page's node identity (its scroll view).
    pub(super) fn id(&self) -> xui::xui_core::backend::WidgetId {
        self.form.id()
    }

    /// Re-lays the page's form.
    pub(super) fn relayout(&self) {
        self.form.relayout();
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.form.set_visible(visible);
    }

    /// Pushes the shared state onto the controls and starts the pack count on
    /// the first call.
    pub(super) fn sync(&mut self, state: &AppState) {
        self.status
            .set_text(&status_text(&state.projectm.availability));
        self.timing.sync(&state.projectm.settings);
        self.ensure_scan();
        self.packs
            .sync(state, &self.exe_dir, self.counts.borrow().as_deref());
        self.packs.sync_helper(self.helper.is_file(), &self.exe_dir);
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

    /// Handles the Visualization page's messages; returns whether `msg` was one.
    pub(super) fn update(
        &mut self,
        msg: &SettingsMsg,
        state: &mut AppState,
        out: &mut Commands,
    ) -> bool {
        match msg {
            SettingsMsg::Viz(edit) => {
                let mut settings = state.projectm.settings.clone();
                form::apply_edit(edit, &mut settings);
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
