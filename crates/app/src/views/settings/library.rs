//! Settings → Library page (#19, #115): the configured root folders, a folder
//! picker to add more, per-folder Remove, and Rescan / Cancel.
//!
//! Folder changes are queued as [`Command`]s; the shell persists them and
//! applies them to the library backend, which triggers the incremental scan
//! shown in the status bar. Adding uses the framework-agnostic off-thread
//! [`folder_picker`](emusic_ui::folder_picker), which the library backend drains
//! on its next tick.

use std::path::PathBuf;

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Commands;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, HasText, Label, ListView};

use crate::app::Msg;

use super::SettingsMsg;
use super::form::{FormPage, HEADING_HEIGHT, ROW_HEIGHT};

/// Height of the folder list, in design units.
const LIST_HEIGHT: f32 = 180.0;
/// Height of the explanatory hint, in design units.
const HINT_HEIGHT: f32 = 40.0;
/// Width of each action button, in design units.
const ACTION_WIDTH: f32 = 130.0;

/// The Library settings page's controls.
///
/// Widgets that are not read after construction (headings, hints, buttons other
/// than Remove) are still owned here: dropping a widget destroys its node.
pub(super) struct LibraryPage {
    form: FormPage,
    _heading: Label<Msg>,
    _hint: Label<Msg>,
    _add: Button<Msg>,
    remove: Button<Msg>,
    _rescan: Button<Msg>,
    _cancel: Button<Msg>,
    list: ListView<Msg>,
    status: Label<Msg>,
    /// The folders the list was last built from, so it is only rebuilt when the
    /// configured set changes.
    applied_folders: Vec<PathBuf>,
}

impl LibraryPage {
    /// Builds the page's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &Ui<Msg>) -> LibraryPage {
        let form = FormPage::new(ui).expect("create library form");

        let heading = Label::new(form.ui(), Rect::default(), "Music folders")
            .expect("create library heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let hint = Label::new(
            form.ui(),
            Rect::default(),
            "Folders scanned for music. Adding or removing one rescans in the background.",
        )
        .expect("create library hint");
        form.add_full(hint.id(), dip(HINT_HEIGHT));

        let actions = form.row(ROW_HEIGHT).expect("create folder action row");
        let add = Button::new(actions.ui(), Rect::default(), "Add folder...")
            .expect("create add-folder button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::AddFolder)));
        let remove = Button::new(actions.ui(), Rect::default(), "Remove")
            .expect("create remove-folder button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::RemoveFolder)));
        let rescan = Button::new(actions.ui(), Rect::default(), "Rescan now")
            .expect("create rescan button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::Rescan)));
        let cancel = Button::new(actions.ui(), Rect::default(), "Cancel scan")
            .expect("create cancel-scan button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::CancelScan)));
        actions
            .fixed(add.id(), dip(ACTION_WIDTH))
            .fixed(remove.id(), dip(ACTION_WIDTH))
            .fixed(rescan.id(), dip(ACTION_WIDTH))
            .fixed(cancel.id(), dip(ACTION_WIDTH))
            .finish();

        let list = ListView::new(form.ui(), Rect::default(), &[])
            .expect("create folder list")
            .on_select(|_| Some(Msg::Settings(SettingsMsg::LibrarySelect)));
        form.add_full(list.id(), dip(LIST_HEIGHT));

        let status = Label::new(form.ui(), Rect::default(), "").expect("create library status");
        form.add_full(status.id(), ROW_HEIGHT);

        LibraryPage {
            form,
            _heading: heading,
            _hint: hint,
            _add: add,
            remove,
            _rescan: rescan,
            _cancel: cancel,
            list,
            status,
            applied_folders: Vec::new(),
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

    /// Rebuilds the folder list when the configured set changed, and mirrors the
    /// scan state onto the buttons.
    pub(super) fn sync(&mut self, state: &AppState, library: &dyn LibraryDataSource) {
        if self.applied_folders != state.library_folders {
            self.applied_folders = state.library_folders.clone();
            let items: Vec<String> = state
                .library_folders
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            let rows: Vec<&str> = items.iter().map(String::as_str).collect();
            self.list.set_items(&rows);
            self.list.select(None);
            self.status.set_text(if state.library_folders.is_empty() {
                "No folders yet. Add one to start scanning."
            } else {
                ""
            });
        }
        self.remove.set_enabled(self.list.selected().is_some());
        self._cancel.set_enabled(library.is_scanning());
    }

    /// Handles the Library page's messages; returns whether `msg` was one.
    pub(super) fn update(&mut self, msg: &SettingsMsg, out: &mut Commands) -> bool {
        match msg {
            SettingsMsg::AddFolder => emusic_ui::folder_picker::request(),
            SettingsMsg::RemoveFolder => {
                if let Some(path) = self
                    .list
                    .selected()
                    .and_then(|index| self.applied_folders.get(index).cloned())
                {
                    out.push(Command::LibraryRemoveFolder(path));
                }
            }
            SettingsMsg::Rescan => out.push(Command::LibraryRescan),
            SettingsMsg::CancelScan => out.push(Command::LibraryCancelScan),
            // The selection mapper only wakes the app so the Remove button
            // re-reads `selected()` in `sync`.
            SettingsMsg::LibrarySelect => {}
            _ => return false,
        }
        true
    }
}
