//! The now-playing surfaces' upcoming queue (#110, #247): a virtual
//! (owner-data) `ListView` over the shared [`QueueRow`] preview.
//!
//! Rows and their indices come from the [`NowPlayingView`] model; this module
//! only owns the native list. The right panel and the central view each pass
//! their own message constructors to [`build`] and [`context_menu`], so the
//! two lists route independently even while both are visible. Double-click or
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
/// activation, a right-click context menu and Delete-to-remove. `on_activate`
/// and `on_context` turn a row index into the caller's own jump/context
/// message, `on_delete` into the caller's own remove message, so the panel and
/// the central view's lists can be told apart. Delete is handled here rather
/// than as a global accelerator so it never steals the key from a focused text
/// field.
pub(super) fn build(
    ui: &mut Ui<Msg>,
    on_activate: impl Fn(usize) -> Option<Msg> + 'static,
    on_context: impl Fn(usize) -> Option<Msg> + 'static,
    on_delete: impl Fn() -> Msg + 'static,
) -> win32ui::Result<ListView<QueueItem, Msg>> {
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
        .on_activate(on_activate)
        .on_context(on_context)
        .on_key(move |key, _modifiers| {
            if key == Key::DELETE {
                Some(on_delete())
            } else {
                None
            }
        });
    Ok(list)
}

/// The queue's right-click menu. `on_remove` builds the caller's own remove
/// message.
#[must_use]
pub(super) fn context_menu(on_remove: impl Fn() -> Msg + 'static) -> Menu<Msg> {
    Menu::new().item("Remove from queue", None, on_remove)
}

/// Builds the list model from the shared preview rows.
pub(super) fn model(model: &Model) -> QueueModel {
    QueueModel {
        rows: model.queue().iter().map(QueueItem::new).collect(),
    }
}
