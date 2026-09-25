//! A reusable virtual Win32 list of names with right-aligned count columns
//! (#248), shared by the Artists view and, next, the Genres view (#250).
//!
//! The list is an owner-data `ListView` over pre-built [`NameCountRow`]s, so
//! thousands of rows cost nothing until painted. The caller decides which rows
//! are shown and when to rebuild them; this module owns the native control, the
//! count columns and the shared "Shuffle play" row context menu.

use std::cell::Cell;
use std::rc::Rc;

use win32ui::prelude::*;
use win32ui::{Control, Fill, ListModel, ListView, Menu, dip};

use crate::app::Msg;

/// A count column of a [`NameCountsView`]: its header and fixed width (in
/// design units). Counts are right-aligned.
pub struct CountColumn {
    /// The header label.
    pub title: &'static str,
    /// The column width in design units.
    pub width: f32,
}

/// One name+counts row, pre-formatted for the virtual list.
pub struct NameCountRow {
    name: String,
    counts: Vec<String>,
}

impl NameCountRow {
    /// Builds a row from its name and one already-formatted string per count
    /// column, in column order.
    pub fn new(name: impl Into<String>, counts: impl IntoIterator<Item = String>) -> Self {
        Self {
            name: name.into(),
            counts: counts.into_iter().collect(),
        }
    }

    /// The row's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The formatted count for `column`, or `""` past the end.
    pub fn count(&self, column: usize) -> &str {
        self.counts
            .as_slice()
            .get(column)
            .map_or("", String::as_str)
    }
}

/// The owner-data model: the rows in display order.
struct NameCountModel {
    rows: Rc<Vec<NameCountRow>>,
}

impl ListModel for NameCountModel {
    type Item = NameCountRow;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> Option<&NameCountRow> {
        self.rows.as_slice().get(index)
    }
}

/// A virtual name+counts list whose row context menu raises
/// [`Msg::NameCountShuffle`].
pub struct NameCountsView {
    list: ListView<NameCountRow, Msg>,
    rows: Rc<Vec<NameCountRow>>,
    context: Menu<Msg>,
    context_row: Cell<Option<usize>>,
}

impl NameCountsView {
    /// Creates the list with a fill-width `name_title` column and the given
    /// right-aligned count columns.
    pub fn new(
        ui: &mut Ui<Msg>,
        name_title: &'static str,
        count_columns: &[CountColumn],
    ) -> Result<Self> {
        let mut list = ListView::new(ui)?
            .column(name_title, Fill, |row: &NameCountRow| row.name.as_str())
            .on_context(|row| Some(Msg::ContextRow(row)));
        for (index, column) in count_columns.iter().enumerate() {
            list = list.column_right(
                column.title,
                dip(column.width),
                move |row: &NameCountRow| row.count(index),
            );
        }

        let context = Menu::new().item("Shuffle play", None, || Msg::NameCountShuffle);
        Ok(Self {
            list,
            rows: Rc::new(Vec::new()),
            context,
            context_row: Cell::new(None),
        })
    }

    /// Applies the current appearance metrics and zebra flag (#309).
    pub fn apply_appearance(&self) {
        crate::appearance::apply_list(&self.list);
    }

    /// Rebuilds the virtual list from `rows`.
    pub fn set_rows(&mut self, rows: Vec<NameCountRow>) {
        self.rows = Rc::new(rows);
        self.list.set_model(NameCountModel {
            rows: Rc::clone(&self.rows),
        });
    }

    /// Remembers the row the context menu was opened on.
    pub fn set_context_row(&self, row: usize) {
        self.context_row.set(Some(row));
    }

    /// The "Shuffle play" row context menu.
    pub fn context_menu(&self) -> &Menu<Msg> {
        &self.context
    }

    /// The name of the row the context menu was opened on, if any.
    pub fn context_name(&self) -> Option<String> {
        self.context_row
            .get()
            .and_then(|index| self.rows.as_slice().get(index))
            .map(|row| row.name.clone())
    }

    /// Shows or hides the list.
    pub fn set_visible(&self, visible: bool) {
        self.list.set_visible(visible);
    }
}

impl AsControl for NameCountsView {
    fn control(&self) -> &Control {
        self.list.control()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_exposes_name_and_counts_in_order() {
        let row = NameCountRow::new("Aphex Twin", ["3".to_string(), "42".to_string()]);
        assert_eq!(row.name(), "Aphex Twin");
        assert_eq!(row.count(0), "3");
        assert_eq!(row.count(1), "42");
        assert_eq!(row.count(2), "", "a missing count is empty");
    }
}
