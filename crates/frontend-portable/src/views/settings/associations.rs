//! Settings → File associations page (#11, #115): a checkbox per extension
//! emusic can play, `Register` / `Unregister all`, and a shortcut to Windows'
//! own Default apps settings.
//!
//! Windows 10/11 does not let an app make itself the default handler
//! programmatically (the per-type `UserChoice` is hash-protected). Registering
//! therefore only makes emusic *available*; on success the Windows Default apps
//! page is opened so the user can confirm emusic for each type. The checkbox
//! selection is pure UI state — it has no effect until `Register` is pressed —
//! so it lives here, not in the shared `AppState`.
//!
//! The registry helpers live in `winshell`, a Windows dependency; another
//! platform gets the page with no extensions and the actions disabled.

use std::cell::{Cell, RefCell};

use xui::xui_core::app::Ui;
use xui::xui_core::backend::WidgetId;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, CheckBox, HasText, Label};

use crate::app::Msg;

use super::SettingsMsg;
use super::form::{FormPage, HEADING_HEIGHT, ROW_HEIGHT};

use crate::make::make;
#[cfg(windows)]
use winshell::assoc::{AssocManager, EXTENSIONS, open_default_apps_settings};
use xui::xui_core::arrange::{button, checkbox, label};

/// The app name the registry keys are written under.
#[cfg(windows)]
const APP_NAME: &str = "emusic";
/// The extensions the checkboxes cover.
#[cfg(windows)]
const EXTENSION_LIST: &[&str] = EXTENSIONS;
/// A non-Windows build has no registry-based associations.
#[cfg(not(windows))]
const EXTENSION_LIST: &[&str] = &[];

/// Checkboxes per row.
const PER_ROW: usize = 4;
/// Width of one extension checkbox, in design units.
const CHECK_WIDTH: f32 = 130.0;
/// Height of the explanatory hint, in design units.
const HINT_HEIGHT: f32 = 56.0;
/// Width of each action button, in design units, matching `actions` order.
const ACTION_WIDTHS: [f32; 5] = [110.0, 110.0, 100.0, 140.0, 230.0];

/// The File associations page's controls.
pub(super) struct AssociationsPage {
    form: FormPage,
    _heading: Label<Msg>,
    _hint: Label<Msg>,
    checks: Vec<CheckBox<Msg>>,
    _actions: Vec<Button<Msg>>,
    status: Label<Msg>,
    /// Checked state for each entry of [`EXTENSION_LIST`], mirrored into
    /// `checks`.
    selected: RefCell<Vec<bool>>,
    /// Whether the registry has been read into the checkboxes yet.
    initialized: Cell<bool>,
}

impl AssociationsPage {
    /// Builds one checkbox per extension plus the action buttons.
    pub(super) fn new(ui: &Ui<Msg>) -> AssociationsPage {
        let form = FormPage::new(ui).expect("create associations form");

        let heading =
            make(form.ui(), label("File associations")).expect("create associations heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let hint = make(
            form.ui(),
            label(
                "Choose which audio file types emusic should open, then Register. Windows does not \
             let an app make itself the default, so Settings opens for you to confirm emusic.",
            ),
        )
        .expect("create associations hint");
        form.add_full(hint.id(), dip(HINT_HEIGHT));

        let mut checks = Vec::with_capacity(EXTENSION_LIST.len());
        for start in (0..EXTENSION_LIST.len()).step_by(PER_ROW) {
            let mut row = form.row(ROW_HEIGHT).expect("create extension row");
            for (index, extension) in EXTENSION_LIST.iter().enumerate().skip(start).take(PER_ROW) {
                let check = make(row.ui(), checkbox(format!(".{extension}")))
                    .expect("create extension checkbox")
                    .on_toggle(move |on| Some(Msg::Settings(SettingsMsg::AssocToggle(index, on))));
                row = row.fixed(check.id(), dip(CHECK_WIDTH));
                checks.push(check);
            }
            row.finish();
        }

        let action_row = form.row(ROW_HEIGHT).expect("create association action row");
        let actions = vec![
            make(action_row.ui(), button("Select all"))
                .expect("create select-all button")
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocSelect(true)))),
            make(action_row.ui(), button("Select none"))
                .expect("create select-none button")
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocSelect(false)))),
            make(action_row.ui(), button("Register"))
                .expect("create register button")
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocRegister))),
            make(action_row.ui(), button("Unregister all"))
                .expect("create unregister button")
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocUnregister))),
            make(action_row.ui(), button("Open Windows Default Apps..."))
                .expect("create open-defaults button")
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocOpenSettings))),
        ];
        let mut action_row = action_row;
        for (button, width) in actions.iter().zip(ACTION_WIDTHS) {
            action_row = action_row.fixed(button.id(), dip(width));
        }
        action_row.finish();

        let status = make(form.ui(), label("")).expect("create associations status");
        form.add_full(status.id(), ROW_HEIGHT);

        AssociationsPage {
            form,
            _heading: heading,
            _hint: hint,
            checks,
            _actions: actions,
            status,
            selected: RefCell::new(vec![false; EXTENSION_LIST.len()]),
            initialized: Cell::new(false),
        }
    }

    /// The page's node identity (its scroll view).
    pub(super) fn id(&self) -> WidgetId {
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

    /// Reads the registry into the checkboxes the first time the page is synced.
    pub(super) fn sync(&self) {
        if self.initialized.replace(true) {
            return;
        }
        #[cfg(windows)]
        {
            let manager = AssocManager::new(APP_NAME);
            let selected: Vec<bool> = EXTENSION_LIST
                .iter()
                .map(|extension| manager.is_registered(extension))
                .collect();
            apply_selection(&self.checks, &selected);
            *self.selected.borrow_mut() = selected;
        }
        #[cfg(not(windows))]
        {
            self.status
                .set_text("File associations are a Windows feature.");
        }
    }

    /// Handles the File associations page's messages; returns whether `msg` was
    /// one.
    pub(super) fn update(&mut self, msg: &SettingsMsg) -> bool {
        match msg {
            SettingsMsg::AssocToggle(index, on) => {
                if let Some(slot) = self.selected.borrow_mut().get_mut(*index) {
                    *slot = *on;
                }
            }
            SettingsMsg::AssocSelect(on) => {
                let mut selected = self.selected.borrow_mut();
                selected.iter_mut().for_each(|slot| *slot = *on);
                apply_selection(&self.checks, &selected);
            }
            SettingsMsg::AssocRegister => self.register(),
            SettingsMsg::AssocUnregister => self.unregister(),
            SettingsMsg::AssocOpenSettings => self.open_settings(),
            _ => return false,
        }
        true
    }

    /// Registers the checked extensions and opens Windows Default apps.
    #[cfg(windows)]
    fn register(&self) {
        let selected = self.selected.borrow();
        let extensions: Vec<&str> = EXTENSION_LIST
            .iter()
            .zip(selected.iter())
            .filter_map(|(extension, &on)| on.then_some(*extension))
            .collect();
        if extensions.is_empty() {
            self.set_error("Select at least one file type to register.");
            return;
        }
        let manager = AssocManager::new(APP_NAME);
        let result = std::env::current_exe()
            .map_err(|err| format!("could not locate emusic.exe: {err}"))
            .and_then(|exe| {
                manager
                    .register(&exe, &extensions)
                    .map_err(|err| err.to_string())
            });
        self.refresh(&manager);
        match result {
            Ok(()) => {
                let _ = open_default_apps_settings(APP_NAME);
                self.status.set_text(&format!(
                    "Registered {} file type{}. Choose emusic in Windows Settings to make it the \
                     default.",
                    extensions.len(),
                    if extensions.len() == 1 { "" } else { "s" }
                ));
            }
            Err(error) => self.set_error(&error),
        }
    }

    /// Registers nothing on a platform without registry associations.
    #[cfg(not(windows))]
    fn register(&self) {
        self.set_error("File associations are a Windows feature.");
    }

    /// Removes every registered extension.
    #[cfg(windows)]
    fn unregister(&self) {
        let manager = AssocManager::new(APP_NAME);
        match manager.unregister() {
            Ok(()) => self
                .status
                .set_text("Removed all emusic file associations."),
            Err(error) => self.set_error(&error.to_string()),
        }
        self.refresh(&manager);
    }

    /// Nothing to unregister off Windows.
    #[cfg(not(windows))]
    fn unregister(&self) {
        self.set_error("File associations are a Windows feature.");
    }

    /// Opens Windows' Default apps page for emusic.
    #[cfg(windows)]
    fn open_settings(&self) {
        match open_default_apps_settings(APP_NAME) {
            Ok(()) => self.status.set_text("Opened Windows Default Apps."),
            Err(error) => self.set_error(&error.to_string()),
        }
    }

    /// No Default apps page off Windows.
    #[cfg(not(windows))]
    fn open_settings(&self) {
        self.set_error("File associations are a Windows feature.");
    }

    /// Re-reads the registration state so the checkboxes match the registry.
    #[cfg(windows)]
    fn refresh(&self, manager: &AssocManager) {
        let selected: Vec<bool> = EXTENSION_LIST
            .iter()
            .map(|extension| manager.is_registered(extension))
            .collect();
        apply_selection(&self.checks, &selected);
        *self.selected.borrow_mut() = selected;
    }

    /// Records an error in the status label and the log.
    fn set_error(&self, message: &str) {
        tracing::warn!(%message, "file association action failed");
        self.status.set_text(&format!("Error: {message}"));
    }
}

/// Mirrors a selection vector onto the checkboxes.
fn apply_selection(checks: &[CheckBox<Msg>], selected: &[bool]) {
    for (check, &on) in checks.iter().zip(selected) {
        check.set_checked(on);
    }
}
