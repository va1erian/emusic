//! The Visualization central view (#338): the portable projectM preset browser.
//!
//! The rows, filter and selection live in the toolkit-agnostic
//! [`PresetBrowser`](emusic_ui::views::preset_browser::PresetBrowser); this
//! module only owns the native controls (a filter box, the current-preset lock,
//! a count label and the model [`ListView`]) and mirrors the model into them.
//! A row's playlist index comes from the scan order, so activating one plays
//! exactly the projectM playlist slot the engine would fill in the same order.
//!
//! The projectM OpenGL surface itself is a separate concern (#301/#371): the
//! portable `xui` GL seam is still Windows-only, so this view is the portable
//! part of the visualization — the browser over the scanned preset list.

mod presets;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use emusic_ui::state::projectm::{PresetRequest, ProjectMSettings};
use emusic_ui::state::{Command, VizCommand};
use emusic_ui::views::preset_browser::PresetBrowser;
use emusic_ui::waker::WakerHandle;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{CheckBox, Edit, Fill, HasText, Label, ListModel, ListView};

use crate::app::Msg;

use crate::make::make;
use presets::{PresetFiles, PresetScanner};
use xui::xui_core::arrange::{checkbox, edit, label, list};

/// The filter/lock band height, in design units.
const HEADER_HEIGHT: f32 = 30.0;
/// Width of the lock toggle, in design units.
const LOCK_WIDTH: f32 = 160.0;
/// Width of the count label, in design units.
const COUNT_WIDTH: f32 = 90.0;
/// Width of the pack column, in design units.
const PACK_WIDTH: f32 = 220.0;
/// Gap between the header controls, in design units.
const GAP: f32 = 8.0;

/// One row of the native list: a scanned preset plus its playlist index.
struct PresetRow {
    name: String,
    pack: String,
    /// The entry's index in the scanned (playlist) order.
    playlist_index: usize,
}

/// The list's model: the visible rows in display order.
struct Rows {
    rows: Rc<Vec<PresetRow>>,
}

impl ListModel for Rows {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        let row = self.rows.get(row)?;
        Some(if column == 0 {
            row.name.as_str()
        } else {
            row.pack.as_str()
        })
    }
}

/// The preset browser view: the filter/lock band over the preset list.
pub struct VisualizationView {
    ui: Ui<Msg>,
    filter: Edit<Msg>,
    lock: CheckBox<Msg>,
    count: Label<Msg>,
    list: ListView<Msg>,
    model: PresetBrowser,
    rows: Rc<Vec<PresetRow>>,
    /// The scanner, once a real scan has been started.
    scanner: Option<PresetScanner>,
    /// The install root a real scan walks.
    exe_dir: PathBuf,
    /// Wake handle the background scan uses when it finishes.
    waker: WakerHandle,
    /// Whether this is a mock run serving deterministic placeholder presets.
    mock: bool,
    /// Whether scanning (or the mock load) has started.
    started: bool,
    /// The model revision last mirrored into the list.
    applied_revision: Cell<u64>,
    /// The lock state last pushed into the toggle.
    applied_lock: Cell<bool>,
    /// The count text last pushed into the label.
    applied_count: RefCell<String>,
    /// The current preset last pushed into the model.
    applied_current: RefCell<Option<PathBuf>>,
}

impl VisualizationView {
    /// Creates the filter box, the lock toggle, the count label and the (empty)
    /// virtualized list.
    pub fn new(ui: &Ui<Msg>, mock: bool, waker: WakerHandle) -> VisualizationView {
        let filter = make(ui, edit())
            .expect("create preset filter")
            .cue("Filter by name or pack")
            .on_change(|text| Some(Msg::PresetFilter(text.to_owned())));
        let lock = make(ui, checkbox("Lock current preset"))
            .expect("create preset lock")
            .on_toggle(|_| Some(Msg::PresetToggleLock));
        let count = make(ui, label("")).expect("create preset count");
        let list = make(ui, list())
            .expect("create preset list")
            .column("Name", Fill)
            .column("Pack", dip(PACK_WIDTH))
            .on_select(|row| Some(Msg::PresetSelect(row)))
            .on_activate(|row| Some(Msg::PresetPlay(row)));
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(PathBuf::from))
            .unwrap_or_default();
        VisualizationView {
            ui: ui.clone(),
            filter,
            lock,
            count,
            list,
            model: PresetBrowser::new(),
            rows: Rc::new(Vec::new()),
            scanner: None,
            exe_dir,
            waker,
            mock,
            started: false,
            applied_revision: Cell::new(u64::MAX),
            applied_lock: Cell::new(false),
            applied_count: RefCell::new(String::new()),
            applied_current: RefCell::new(None),
        }
    }

    /// Moves and sizes the view: the filter/lock band over the list.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let header = dip(HEADER_HEIGHT).to_px(dpi).value();
        let gap = dip(GAP).to_px(dpi).value();
        let lock_w = dip(LOCK_WIDTH).to_px(dpi).value();
        let count_w = dip(COUNT_WIDTH).to_px(dpi).value();
        let top = bounds.top;
        let count_left = bounds.right - count_w;
        let lock_left = count_left - gap - lock_w;
        self.ui.apply_moves(&[
            (
                self.filter.id(),
                Rect::new(bounds.left, top, lock_left - gap, top + header),
            ),
            (
                self.lock.id(),
                Rect::new(lock_left, top, count_left - gap, top + header),
            ),
            (
                self.count.id(),
                Rect::new(count_left, top, bounds.right, top + header),
            ),
        ]);
        self.ui.apply_moves(&[(
            self.list.id(),
            Rect::new(bounds.left, top + header, bounds.right, bounds.bottom),
        )]);
    }

    /// Shows or hides the whole view.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.filter.id(), visible);
        self.ui.set_visible(self.lock.id(), visible);
        self.ui.set_visible(self.count.id(), visible);
        self.ui.set_visible(self.list.id(), visible);
    }

    /// Starts the scan on the first call, picks up its result, then mirrors the
    /// settings and the model into the controls.
    pub fn sync(&mut self, settings: &ProjectMSettings) {
        if !self.started {
            self.started = true;
            if self.mock {
                self.model.rebuild(&PresetFiles::placeholder().presets);
            } else {
                self.scanner = Some(PresetScanner::spawn(
                    &self.exe_dir,
                    settings,
                    self.waker.clone(),
                ));
            }
        }
        if let Some(scanner) = self.scanner.as_ref()
            && let Some(files) = scanner.try_take()
        {
            self.model.rebuild(&files.presets);
            self.scanner = None;
        }

        // Guard every push on an actual change: `sync` runs every tick, and an
        // unconditional `set_text`/`set_current` would invalidate the widgets
        // (and clone the path) each time.
        if self.applied_current.borrow().as_ref() != settings.last_preset.as_ref() {
            *self.applied_current.borrow_mut() = settings.last_preset.clone();
            self.model.set_current(settings.last_preset.as_deref());
        }
        if settings.preset_locked != self.applied_lock.get() {
            self.applied_lock.set(settings.preset_locked);
            self.lock.set_checked(settings.preset_locked);
        }
        let count = format!("{} of {}", self.model.len(), self.model.total());
        if *self.applied_count.borrow() != count {
            self.count.set_text(&count);
            *self.applied_count.borrow_mut() = count;
        }

        if self.model.revision() != self.applied_revision.get() {
            self.applied_revision.set(self.model.revision());
            self.rebuild_rows();
        }
    }

    /// Handles the filter box: replaces the model filter, which the next
    /// [`Self::sync`] mirrors into the list.
    pub fn set_filter(&mut self, filter: &str) {
        self.model.set_filter(filter);
    }

    /// Remembers the selected visible row; the native list already reflects it.
    pub fn select(&mut self, row: usize) {
        self.model.select(Some(row));
    }

    /// The playlist index to play for visible `row`, or `None` past the end.
    pub fn playlist_index(&self, row: usize) -> Option<usize> {
        self.rows.as_slice().get(row).map(|row| row.playlist_index)
    }

    /// Rebuilds the native rows from the model's visible entries and restores
    /// the user's selection (falling back to the current preset, e.g. on the
    /// first build), so a filter edit does not lose it.
    fn rebuild_rows(&mut self) {
        let rows: Vec<PresetRow> = (0..self.model.len())
            .filter_map(|row| {
                let entry = self.model.entry(row)?;
                let playlist_index = self.model.playlist_index(row)?;
                Some(PresetRow {
                    name: entry.name.clone(),
                    pack: entry.pack.clone(),
                    playlist_index,
                })
            })
            .collect();
        self.rows = Rc::new(rows);
        self.list.set_model(Rows {
            rows: Rc::clone(&self.rows),
        });
        match self.model.selected().or_else(|| self.model.current_row()) {
            Some(row) => self.list.select(Some(row)),
            None => self.list.select(None),
        }
    }
}

/// The two commands a preset activation produces: make sure the visualization
/// is showing, then play the preset at `index`.
pub fn play_commands(index: usize) -> [Command; 2] {
    [
        Command::Viz(VizCommand::SetVisible(true)),
        Command::Viz(VizCommand::Preset(PresetRequest::Index(index))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_commands_show_then_play_the_index() {
        let commands = play_commands(7);
        assert_eq!(commands[0], Command::Viz(VizCommand::SetVisible(true)));
        assert_eq!(
            commands[1],
            Command::Viz(VizCommand::Preset(PresetRequest::Index(7)))
        );
    }
}
