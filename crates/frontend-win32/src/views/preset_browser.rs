//! Win32 preset browser (#338): a virtualized [`ListView`] of every scanned
//! projectM preset, with a filter box and the current-preset lock.
//!
//! The rows, filter and selection live in the toolkit-agnostic
//! [`PresetBrowser`](emusic_ui::views::preset_browser::PresetBrowser); this
//! module only owns the native controls and mirrors the model into them. A
//! row's playlist index comes from the scan order, so activating one plays
//! exactly the projectM playlist slot the engine filled in the same order.

use std::cell::Cell;
use std::rc::Rc;

use emusic_ui::state::projectm::{PresetRequest, ProjectMSettings};
use emusic_ui::state::{Command, VizCommand};
use emusic_ui::views::preset_browser::PresetBrowser;
use win32ui::prelude::*;
use win32ui::{
    CheckBox, Control, Edit, Label, Layout, ListModel, ListView, RowStyle, column, dip, row,
};

use crate::app::Msg;
use crate::views::projectm::PresetFiles;

/// The filter/lock band height, in design units.
const HEADER_HEIGHT: f32 = 30.0;
/// Width of the lock toggle, in design units.
const LOCK_WIDTH: f32 = 160.0;
/// Width of the count label, in design units.
const COUNT_WIDTH: f32 = 90.0;
/// Width of the pack column, in design units.
const PACK_WIDTH: f32 = 220.0;

/// One row of the native list: a scanned preset plus its playlist index.
struct PresetRow {
    name: String,
    pack: String,
    /// The entry's index in the scanned (playlist) order.
    playlist_index: usize,
}

/// The owner-data model: the visible rows, shared with the view so the row
/// style can follow the current preset without rebuilding.
struct Rows {
    rows: Rc<Vec<PresetRow>>,
}

impl ListModel for Rows {
    type Item = PresetRow;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> Option<&PresetRow> {
        self.rows.as_slice().get(index)
    }
}

/// The Win32 preset browser: the filter/lock band over the preset list.
pub struct PresetBrowserView {
    filter: Edit<Msg>,
    lock: CheckBox<Msg>,
    count: Label,
    list: ListView<PresetRow, Msg>,
    /// The toolkit-agnostic rows/filter/selection model.
    model: PresetBrowser,
    /// The visible rows handed to the native list, so a click can be mapped
    /// back to its playlist index.
    rows: Rc<Vec<PresetRow>>,
    /// The playlist index of the currently showing preset, shared with the row
    /// style so the highlight follows playback without a rebuild.
    current: Rc<Cell<Option<usize>>>,
    /// The scanner generation last mirrored, so a rescan rebuilds the list.
    applied_scan: Cell<u64>,
    /// The model revision last mirrored.
    applied_revision: Cell<u64>,
    /// The lock state last pushed into the toggle.
    applied_lock: Cell<bool>,
    /// Whether the current preset was already revealed for this visit, so
    /// opening the view scrolls to it exactly once.
    revealed: Cell<bool>,
}

impl PresetBrowserView {
    /// Creates the filter box, the lock toggle, the count label and the
    /// (empty) virtualized list.
    pub fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let filter = Edit::single_line(ui)?
            .cue("Filter by name or pack")
            .on_change(|text| Some(Msg::PresetFilter(text.to_owned())));
        let lock = CheckBox::new(ui, "Lock current preset")?
            .on_toggle(|_| Some(Msg::Viz(VizCommand::TogglePresetLock)));
        let count = Label::new(ui, Rect::default(), "")?;

        let current = Rc::new(Cell::new(None));
        let current_for_style = Rc::clone(&current);
        let current_for_name = Rc::clone(&current);
        let list = ListView::new(ui)?
            .row_style(move |row: &PresetRow| {
                if current_for_style.get() == Some(row.playlist_index) {
                    RowStyle::new().bold(true)
                } else {
                    RowStyle::default()
                }
            })
            .add_column(
                win32ui::Column::new("Name", Fill, |row: &PresetRow| row.name.as_str()).cell_color(
                    move |row, theme| {
                        (current_for_name.get() == Some(row.playlist_index)).then_some(theme.accent)
                    },
                ),
            )
            .column("Pack", dip(PACK_WIDTH), |row: &PresetRow| row.pack.as_str())
            .on_select(|rows| rows.first().copied().map(Msg::PresetSelect))
            .on_activate(|row| Some(Msg::PresetPlay(row)));

        Ok(Self {
            filter,
            lock,
            count,
            list,
            model: PresetBrowser::new(),
            rows: Rc::new(Vec::new()),
            current,
            applied_scan: Cell::new(u64::MAX),
            applied_revision: Cell::new(u64::MAX),
            applied_lock: Cell::new(false),
            revealed: Cell::new(false),
        })
    }

    /// Applies the current appearance metrics and zebra flag (#309).
    pub fn apply_appearance(&self) {
        crate::appearance::apply_list(&self.list);
    }

    /// Pushes the scanned preset file list, the settings and the current
    /// preset into the view. `scan` is bumped by the app whenever a new scan
    /// finishes, so the list is only rebuilt then (never per keystroke).
    pub(crate) fn sync(
        &mut self,
        settings: &ProjectMSettings,
        files: Option<&PresetFiles>,
        scan: u64,
    ) {
        if self.applied_scan.get() != scan {
            self.applied_scan.set(scan);
            let entries = files.map(|files| files.presets.as_slice()).unwrap_or(&[]);
            self.model.rebuild(entries);
        }
        self.model.set_current(settings.last_preset.as_deref());
        let current = self.model.current_playlist_index();
        if self.current.replace(current) != current && !self.rows.is_empty() {
            // The row style reads `current` during paint, so a preset change
            // must repaint the visible rows to move the highlight.
            self.list.rows_changed(0..self.rows.len());
        }

        if settings.preset_locked != self.applied_lock.get() {
            self.applied_lock.set(settings.preset_locked);
            self.lock.set_checked(settings.preset_locked);
        }

        self.count
            .set_text(&format!("{} of {}", self.model.len(), self.model.total()));

        if self.applied_revision.get() != self.model.revision() {
            self.applied_revision.set(self.model.revision());
            self.rebuild_rows();
        }

        // Reveal the current preset once per visit, but only after a scan has
        // produced a list — otherwise a browser opened before the scan finishes
        // would latch with nothing to scroll to.
        if !self.revealed.get() && (self.reveal_current() || files.is_some()) {
            self.revealed.set(true);
        }
    }

    /// Selects and scrolls to the current preset, if it is visible. Returns
    /// whether it scrolled.
    fn reveal_current(&mut self) -> bool {
        match self.model.current_row() {
            Some(row) => {
                self.list.select(row);
                self.list.ensure_visible(row);
                self.model.select(Some(row));
                true
            }
            None => false,
        }
    }

    /// Handles the filter box: replaces the model filter, which the next
    /// [`Self::sync`] mirrors into the list.
    pub fn set_filter(&mut self, filter: &str) {
        self.model.set_filter(filter);
    }

    /// Remembers the selected visible row (the native list already reflects
    /// it; this only keeps the model's selection across a rebuild).
    pub fn select(&mut self, row: usize) {
        self.model.select(Some(row));
    }

    /// The playlist index to play for visible `row`, or `None` past the end.
    pub fn playlist_index(&self, row: usize) -> Option<usize> {
        self.rows.as_slice().get(row).map(|row| row.playlist_index)
    }

    /// Shows or hides the whole view. Leaving resets the reveal latch so the
    /// next visit scrolls back to the current preset.
    pub fn set_visible(&self, visible: bool) {
        self.filter.set_visible(visible);
        self.lock.set_visible(visible);
        self.count.set_visible(visible);
        self.list.set_visible(visible);
        if !visible {
            self.revealed.set(false);
        }
    }

    /// The filter/lock band over the virtualized preset list.
    pub fn layout(&self) -> Layout {
        column![
            row![
                self.filter.fill(1),
                self.lock.width(dip(LOCK_WIDTH)),
                self.count.width(dip(COUNT_WIDTH)),
            ]
            .spacing(dip(8.0))
            .height(dip(HEADER_HEIGHT)),
            self.list.fill(1),
        ]
    }

    /// Rebuilds the native rows from the model's visible entries.
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
        // Restore the selection the model remembers, so a rescan or a filter
        // change keeps the same preset selected when it is still visible.
        match self.model.selected() {
            Some(row) => self.list.select(row),
            None => self.list.set_selection(&[]),
        }
    }
}

impl AsControl for PresetBrowserView {
    fn control(&self) -> &Control {
        self.list.control()
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
        assert_eq!(
            commands[0],
            Command::Viz(VizCommand::SetVisible(true)),
            "playing a preset must make the visualization visible"
        );
        assert_eq!(
            commands[1],
            Command::Viz(VizCommand::Preset(PresetRequest::Index(7)))
        );
    }
}
