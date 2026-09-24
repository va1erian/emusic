//! The now-playing panel's upcoming queue (#110): a virtual (owner-data)
//! `ListView` over the shared [`QueueRow`] preview.
//!
//! Rows and their indices come from the [`NowPlayingView`] model; this module
//! only owns the native list and maps its events to [`Msg`]s. Double-click or
//! Enter jumps to a queue entry, right-click opens a "Remove" context menu.

use emusic_ui::views::now_playing::{NowPlayingView as Model, QueueRow};
use win32ui::prelude::*;
use win32ui::{ColumnWidth, Fill, ListView, Menu, dip};

use crate::app::Msg;

/// Height of the queue list, in device-independent pixels.
pub(super) const QUEUE_HEIGHT: f32 = 200.0;
/// Width of the row-number column.
const NUMBER_WIDTH: f32 = 28.0;
/// Width of the artist column.
const ARTIST_WIDTH: f32 = 108.0;

/// One queue row with its number pre-formatted, so owner-data requests never
/// allocate.
#[derive(Clone)]
pub(super) struct QueueItem {
    /// 1-based position text (`"3."`).
    number_text: String,
    title: String,
    artist: String,
}

impl QueueItem {
    fn new(row: &QueueRow) -> Self {
        Self {
            number_text: format!("{}.", row.number),
            title: row.title.clone(),
            artist: row.artist.clone(),
        }
    }
}

/// The queue list model: the preview rows in display order.
pub(super) struct QueueModel {
    rows: Vec<QueueItem>,
}

impl ListModel for QueueModel {
    type Item = QueueItem;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> Option<&QueueItem> {
        self.rows.as_slice().get(index)
    }
}

/// Builds the queue list: number, title and artist columns, with double-click
/// activation and a right-click context menu.
pub(super) fn build(ui: &mut Ui<Msg>) -> win32ui::Result<ListView<QueueItem, Msg>> {
    let list = ListView::new(ui)?
        .column("#", dip(NUMBER_WIDTH), |row: &QueueItem| {
            row.number_text.as_str()
        })
        .column("Title", Fill, |row: &QueueItem| row.title.as_str())
        .column(
            "Artist",
            ColumnWidth::Fixed(dip(ARTIST_WIDTH)),
            |row: &QueueItem| row.artist.as_str(),
        )
        .on_activate(|row| Some(Msg::QueueJump(row)))
        .on_context(|row| Some(Msg::QueueContext(row)));
    Ok(list)
}

/// The queue's right-click menu.
#[must_use]
pub(super) fn context_menu() -> Menu<Msg> {
    Menu::new().item("Remove from queue", None, || Msg::QueueRemove)
}

/// Builds the list model from the shared preview rows.
pub(super) fn model(model: &Model) -> QueueModel {
    QueueModel {
        rows: model.queue().iter().map(QueueItem::new).collect(),
    }
}
