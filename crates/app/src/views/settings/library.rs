//! Settings → Library page (#19, #115): the configured root folders, a native
//! folder picker to add more, per-folder Remove, and Rescan / Cancel.
//!
//! Changes are queued as [`Command`]s;
//! the shell persists them to the config and applies them to the library
//! backend, which triggers the incremental scan shown in the status bar.

use std::path::PathBuf;

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Commands;
use xui::prelude::*;
use xui::{Button, Fill, ListView, Proxy, dip};

use crate::app::Msg;

use super::{FormRow, HEADING_HEIGHT, ROW_HEIGHT, ScrollPanel, SettingsMsg};

/// Height of the folder list, in design units.
const LIST_HEIGHT: f32 = 180.0;
/// Width of each action button, in design units.
const ACTION_WIDTH: f32 = 130.0;

/// The list model: one display string per configured folder, in config order.
struct FolderModel {
    rows: Vec<String>,
}

impl ListModel for FolderModel {
    type Item = String;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> Option<&String> {
        self.rows.as_slice().get(index)
    }
}

/// The Library settings page's controls.
pub(super) struct LibraryPage {
    form: ScrollPanel,
    heading: Label,
    hint: Label,
    add: Button<Msg>,
    remove: Button<Msg>,
    rescan: Button<Msg>,
    cancel: Button<Msg>,
    list: ListView<String, Msg>,
    status: Label,
    proxy: Proxy<Msg>,
    /// The folders the list was last built from, so it is only rebuilt when the
    /// configured set changes.
    applied_folders: Vec<PathBuf>,
}

impl LibraryPage {
    /// Builds the page's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>, proxy: Proxy<Msg>) -> xui::Result<Self> {
        let form = ScrollPanel::new(ui)?;
        let mut panel = form.ui(ui);
        let heading = Label::new(&mut panel, Rect::default(), "Music folders")?;
        let hint = Label::new(
            &mut panel,
            Rect::default(),
            "Folders scanned for music. Adding or removing one rescans in the background.",
        )?;
        let add = Button::new(&mut panel, "Add folder...")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::AddFolder)));
        let remove = Button::new(&mut panel, "Remove")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::RemoveFolder)));
        let rescan = Button::new(&mut panel, "Rescan now")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::Rescan)));
        let cancel = Button::new(&mut panel, "Cancel scan")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::CancelScan)));
        let list = ListView::new(&mut panel)?.column("Folder", Fill, |row: &String| row.as_str());
        let status = Label::new(&mut panel, Rect::default(), "")?;

        let page = Self {
            form,
            heading,
            hint,
            add,
            remove,
            rescan,
            cancel,
            list,
            status,
            proxy,
            applied_folders: Vec::new(),
        };
        page.apply(ui);
        Ok(page)
    }

    /// The page's scrollable form as one tab-strip page.
    pub(super) fn page(&self) -> LayoutItem {
        self.form.page()
    }

    /// The page's controls as form rows, in display order.
    fn rows(&self) -> Vec<FormRow> {
        let mut actions = Layout::row().spacing(dip(8.0));
        for button in [&self.add, &self.remove, &self.rescan, &self.cancel] {
            actions = actions.item(button.width(dip(ACTION_WIDTH)));
        }
        vec![
            (self.heading.height(dip(HEADING_HEIGHT)), HEADING_HEIGHT),
            (self.hint.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (actions.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (self.list.height(dip(LIST_HEIGHT)), LIST_HEIGHT),
            (self.status.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
        ]
    }

    /// Reinstalls the page's form (used after a visibility change).
    fn apply(&self, ui: &Ui<Msg>) {
        self.form.apply(ui, self.rows());
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.form.set_visible(visible);
    }

    /// Applies the current appearance metrics and zebra flag (#309).
    pub(super) fn apply_appearance(&self) {
        crate::appearance::apply_list(&self.list);
    }

    /// Rebuilds the folder list when the configured set changed, and mirrors
    /// the scan state onto the buttons.
    pub(super) fn sync(&mut self, state: &AppState, library: &dyn LibraryDataSource) {
        if self.applied_folders != state.library_folders {
            self.applied_folders = state.library_folders.clone();
            let rows = state
                .library_folders
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            self.list.set_model(FolderModel { rows });
            self.status.set_text(if state.library_folders.is_empty() {
                "No folders yet. Add one to start scanning."
            } else {
                ""
            });
            self.list.set_selection(&[]);
        }
        self.remove.set_enabled(self.list.selected().is_some());
        self.cancel.set_enabled(library.is_scanning());
    }

    /// Handles the Library page's messages; returns whether `msg` was one.
    pub(super) fn update(&mut self, msg: &SettingsMsg, out: &mut Commands) -> bool {
        match msg {
            SettingsMsg::AddFolder => {
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let picked = rfd::FileDialog::new().pick_folder();
                    let _ = proxy.send(Msg::Settings(SettingsMsg::FolderPicked(picked)));
                });
            }
            SettingsMsg::FolderPicked(Some(path)) => {
                if !self.applied_folders.contains(path) {
                    out.push(Command::LibraryAddFolder(path.clone()));
                }
            }
            SettingsMsg::FolderPicked(None) => {}
            SettingsMsg::RemoveFolder => {
                if let Some(path) = self
                    .list
                    .selected()
                    .and_then(|index| self.applied_folders.as_slice().get(index).cloned())
                {
                    out.push(Command::LibraryRemoveFolder(path));
                }
            }
            SettingsMsg::Rescan => out.push(Command::LibraryRescan),
            SettingsMsg::CancelScan => out.push(Command::LibraryCancelScan),
            _ => return false,
        }
        true
    }
}
