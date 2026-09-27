//! The now-playing upcoming queue (#110, #247): a portable model [`ListView`]
//! over the shared queue preview.
//!
//! Rows and their entry indices come from the
//! [`NowPlayingView`](emusic_ui::views::now_playing::NowPlayingView) model; this
//! module only owns the native list. Double-clicking or pressing Enter on a row
//! raises [`Msg::QueueActivate`] with the visible row, which the view maps back
//! to the full queue entry index.

use std::rc::Rc;

use emusic_ui::views::now_playing::QueueRow;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Fill, ListModel, ListView};

use crate::app::Msg;

/// Width of the row-number column, in design units.
const NUMBER_WIDTH: f32 = 28.0;
/// Width of the artist column, in design units.
const ARTIST_WIDTH: f32 = 108.0;

/// One queue row with its number pre-formatted, so the paint path allocates
/// nothing.
struct QueueItem {
    number: String,
    title: String,
    artist: String,
}

/// The list's model: the preview rows in display order.
struct QueueModel {
    rows: Rc<Vec<QueueItem>>,
}

impl ListModel for QueueModel {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        let item = self.rows.get(row)?;
        Some(match column {
            0 => item.number.as_str(),
            1 => item.title.as_str(),
            _ => item.artist.as_str(),
        })
    }
}

/// The upcoming-queue list.
pub struct QueueList {
    ui: Ui<Msg>,
    list: ListView<Msg>,
    /// The full queue entry index of each preview row.
    indices: Rc<Vec<usize>>,
}

impl QueueList {
    /// Creates the empty queue list.
    pub fn new(ui: &Ui<Msg>) -> QueueList {
        let list = ListView::new(ui, Rect::default(), &[])
            .expect("create now-playing queue list")
            .column("#", dip(NUMBER_WIDTH))
            .column("Title", Fill)
            .column("Artist", dip(ARTIST_WIDTH))
            .on_activate(|row| Some(Msg::QueueActivate(row)));
        QueueList {
            ui: ui.clone(),
            list,
            indices: Rc::new(Vec::new()),
        }
    }

    /// Replaces the rows from the model's queue preview.
    pub fn sync(&mut self, model: &[QueueRow]) {
        let items: Vec<QueueItem> = model
            .iter()
            .map(|row| QueueItem {
                number: format!("{}.", row.number),
                title: row.title.clone(),
                artist: row.artist.clone(),
            })
            .collect();
        self.indices = Rc::new(model.iter().map(|row| row.index).collect());
        self.list.set_model(QueueModel {
            rows: Rc::new(items),
        });
        // `set_model` focuses the last row; the queue starts with none
        // selected, matching the pre-migration panel.
        self.list.set_selection(&[]);
    }

    /// The full queue entry index of preview `row`, if it exists.
    pub fn entry_index(&self, row: usize) -> Option<usize> {
        self.indices.as_slice().get(row).copied()
    }

    /// Moves/resizes the list.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.list.id(), rect)]);
    }

    /// Shows or hides the list.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.list.id(), visible);
    }
}
