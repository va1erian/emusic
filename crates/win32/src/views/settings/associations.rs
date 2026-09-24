//! Settings → File associations page (#11, #115): a checkbox per extension
//! emusic can play, `Register` / `Unregister all`, and a shortcut to Windows'
//! own Default apps settings.
//!
//! Windows 10/11 does not let an app make itself the default handler
//! programmatically (the per-type `UserChoice` is hash-protected and guarded by
//! `UCPD.sys`). Registering therefore only makes emusic *available*; on success
//! the Windows Default apps page is opened so the user can confirm emusic for
//! each type. The checkbox selection is pure UI state — it has no effect until
//! `Register` is pressed — so it lives here, not in the shared `AppState`.

use std::cell::{Cell, RefCell};

use win32ui::prelude::*;
use win32ui::{Button, CheckBox, dip};
use winshell::assoc::{AssocManager, EXTENSIONS, open_default_apps_settings};

use crate::app::Msg;

use super::{HEADING_HEIGHT, ROW_HEIGHT, SettingsMsg};

/// The app name the registry keys are written under.
const APP_NAME: &str = "emusic";
/// Checkboxes per row (a native form grid).
const PER_ROW: usize = 4;
/// Width of one extension checkbox, in design units.
const CHECK_WIDTH: f32 = 130.0;
/// Width of each action button, in design units, matching `actions` order.
const ACTION_WIDTHS: [f32; 5] = [110.0, 110.0, 100.0, 140.0, 230.0];

/// The File associations page's controls.
pub(super) struct AssociationsPage {
    heading: Label,
    hint: Label,
    checks: Vec<CheckBox<Msg>>,
    actions: Vec<Button<Msg>>,
    status: Label,
    /// Checked state for each entry of [`EXTENSIONS`], mirrored into `checks`.
    selected: RefCell<Vec<bool>>,
    /// Whether the registry has been read into the checkboxes yet.
    initialized: Cell<bool>,
}

impl AssociationsPage {
    /// Builds one checkbox per extension plus the action buttons.
    pub(super) fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let mut checks = Vec::with_capacity(EXTENSIONS.len());
        for (index, ext) in EXTENSIONS.iter().enumerate() {
            let check = CheckBox::new(ui, &format!(".{ext}"))?
                .on_toggle(move |on| Some(Msg::Settings(SettingsMsg::AssocToggle(index, on))));
            checks.push(check);
        }

        let actions = vec![
            Button::new(ui, "Select all")?
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocSelect(true)))),
            Button::new(ui, "Select none")?
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocSelect(false)))),
            Button::new(ui, "Register")?
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocRegister))),
            Button::new(ui, "Unregister all")?
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocUnregister))),
            Button::new(ui, "Open Windows Default Apps...")?
                .on_click(|| Some(Msg::Settings(SettingsMsg::AssocOpenSettings))),
        ];

        Ok(Self {
            heading: Label::new(ui, Rect::default(), "File associations")?,
            hint: Label::new(
                ui,
                Rect::default(),
                "Choose which audio file types emusic should open, then Register. Windows 10/11 does not let an app make itself the default, so Settings opens for you to confirm emusic for each type.",
            )?,
            checks,
            actions,
            status: Label::new(ui, Rect::default(), "")?,
            selected: RefCell::new(Vec::new()),
            initialized: Cell::new(false),
        })
    }

    /// The page's controls as layout items, in display order.
    pub(super) fn items(&self) -> Vec<LayoutItem> {
        let mut items = vec![
            self.heading.height(dip(HEADING_HEIGHT)),
            self.hint.height(dip(ROW_HEIGHT * 2.0)),
        ];
        for chunk in self.checks.chunks(PER_ROW) {
            let mut row = Layout::row().spacing(dip(8.0));
            for check in chunk {
                row = row.item(check.width(dip(CHECK_WIDTH)));
            }
            items.push(row.height(dip(ROW_HEIGHT)));
        }
        let mut actions = Layout::row().spacing(dip(8.0));
        for (button, width) in self.actions.iter().zip(ACTION_WIDTHS) {
            actions = actions.item(button.width(dip(width)));
        }
        items.push(actions.height(dip(ROW_HEIGHT)));
        items.push(self.status.height(dip(ROW_HEIGHT)));
        items
    }

    /// Shows or hides every control on the page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.heading.set_visible(visible);
        self.hint.set_visible(visible);
        for check in &self.checks {
            check.set_visible(visible);
        }
        for button in &self.actions {
            button.set_visible(visible);
        }
        self.status.set_visible(visible);
    }

    /// Reads the registry into the checkboxes the first time the page is synced.
    pub(super) fn sync(&self) {
        if self.initialized.replace(true) {
            return;
        }
        let manager = AssocManager::new(APP_NAME);
        let selected: Vec<bool> = EXTENSIONS
            .iter()
            .map(|ext| manager.is_registered(ext))
            .collect();
        apply_selection(&self.checks, &selected);
        *self.selected.borrow_mut() = selected;
    }

    /// Handles the File associations page's messages.
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
            SettingsMsg::AssocOpenSettings => {
                if let Err(err) = open_default_apps_settings(APP_NAME) {
                    self.set_error(&err.to_string());
                } else {
                    self.status.set_text("Opened Windows Default Apps.");
                }
            }
            _ => return false,
        }
        true
    }

    /// Registers the checked extensions and opens Windows Default apps.
    fn register(&self) {
        let selected = self.selected.borrow();
        let extensions: Vec<&str> = EXTENSIONS
            .iter()
            .zip(selected.iter())
            .filter_map(|(ext, &on)| on.then_some(*ext))
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
                    "Registered {} file type{}. Choose emusic in Windows Settings to make it the default.",
                    extensions.len(),
                    if extensions.len() == 1 { "" } else { "s" }
                ));
            }
            Err(err) => self.set_error(&err),
        }
    }

    /// Removes every registered extension.
    fn unregister(&self) {
        let manager = AssocManager::new(APP_NAME);
        match manager.unregister() {
            Ok(()) => self
                .status
                .set_text("Removed all emusic file associations."),
            Err(err) => self.set_error(&err.to_string()),
        }
        self.refresh(&manager);
    }

    /// Re-reads the registration state so the checkboxes match the registry.
    fn refresh(&self, manager: &AssocManager) {
        let selected: Vec<bool> = EXTENSIONS
            .iter()
            .map(|ext| manager.is_registered(ext))
            .collect();
        apply_selection(&self.checks, &selected);
        *self.selected.borrow_mut() = selected;
    }

    /// Records an error in the status label (and the log).
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
