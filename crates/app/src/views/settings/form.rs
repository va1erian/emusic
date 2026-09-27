//! A scrollable Settings form page on the portable [`ScrollView`].
//!
//! `xui_core`'s `ScrollView` stacks one widget per full-width row; a form needs
//! "label | field" rows too, so [`FormPage`] adds two kinds of row:
//!
//! - [`FormPage::add_full`] registers a single widget as a full-width row
//!   (headings, checkboxes, a [`RadioGroup`] option, a [`ListView`]).
//! - [`FormPage::row`] wraps several widgets in a plain container and lays them
//!   out left to right, each either a fixed width or filling the remainder
//!   ([`RowBuilder::fixed`] / [`RowBuilder::fill`]).
//!
//! The scroll extent is the sum of the row heights, so the scrollbar matches
//! exactly. Re-laying the children after the host has positioned a row keeps the
//! form correct on resize and on the first layout after a tab becomes active.

use std::cell::RefCell;

use xui::xui_core::app::Ui;
use xui::xui_core::backend::{NodeKind, NodeSpec, Result, WidgetId};
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::{Dip, dip};
use xui::xui_core::widget::ScrollView;

use crate::app::Msg;

/// Height of a standard form row (label + field), in design units.
pub const ROW_HEIGHT: Dip = dip(28.0);
/// Height of a section heading, in design units.
pub const HEADING_HEIGHT: Dip = dip(24.0);
/// Width of the label column in a "label | field" row, in design units.
pub const LABEL_WIDTH: Dip = dip(160.0);
/// Gap between the widgets of one row, in design units.
const GAP: Dip = dip(8.0);

/// How much of a row a widget takes.
enum ItemWidth {
    /// A fixed design width.
    Fixed(Dip),
    /// An equal share of what the fixed widths leave.
    Fill,
}

/// One registered row: the container node the host scrolls, and the widgets to
/// place inside it (empty for a full-width single-widget row).
struct Row {
    node: WidgetId,
    height: Dip,
    items: Vec<(WidgetId, ItemWidth)>,
}

/// A page's form: the scroll host plus its registered rows.
pub struct FormPage {
    scroll: ScrollView<Msg>,
    scoped: Ui<Msg>,
    rows: RefCell<Vec<Row>>,
}

impl FormPage {
    /// Creates an empty scrollable form page.
    pub fn new(ui: &Ui<Msg>) -> Result<FormPage> {
        let scroll = ScrollView::new(ui, Rect::default())?;
        let scoped = scroll.ui().clone();
        Ok(FormPage {
            scroll,
            scoped,
            rows: RefCell::new(Vec::new()),
        })
    }

    /// A [`Ui`] scoped to the page, so widgets created through it are children
    /// of the scroll view and scroll with it.
    pub fn ui(&self) -> &Ui<Msg> {
        &self.scoped
    }

    /// Registers `id` as a full-width row `height` design values tall.
    pub fn add_full(&self, id: WidgetId, height: Dip) {
        self.rows.borrow_mut().push(Row {
            node: id,
            height,
            items: Vec::new(),
        });
        self.scroll.add(id, height);
    }

    /// Starts a multi-widget row of `height`; finish it with
    /// [`RowBuilder::finish`].
    pub fn row(&self, height: Dip) -> Result<RowBuilder<'_>> {
        let node = self.scoped.create_child(
            self.scroll.id(),
            &NodeSpec::new(NodeKind::Container, Rect::default()),
        )?;
        let ui = self.scoped.with_parent(node);
        Ok(RowBuilder {
            page: self,
            node,
            ui,
            height,
            items: Vec::new(),
        })
    }

    /// The page's node identity (the scroll view).
    pub fn id(&self) -> WidgetId {
        self.scroll.id()
    }

    /// Shows or hides the whole page.
    pub fn set_visible(&self, visible: bool) {
        self.scroll.set_visible(visible);
    }

    /// Re-lays every multi-widget row from the host's current row bounds.
    pub fn relayout(&self) {
        self.scroll.relayout();
        if std::env::var_os("EMUSIC_SETTINGS_DEBUG").is_some() {
            eprintln!(
                "FormPage scroll={:?}",
                self.scoped.bounds(self.scroll.id())
            );
            for row in self.rows.borrow().iter() {
                eprintln!(
                    "  row node={:?} bounds={:?} h={:?} items={}",
                    row.node,
                    self.scoped.bounds(row.node),
                    row.height,
                    row.items.len()
                );
                for (id, _) in row.items.iter() {
                    eprintln!("    item {:?} bounds={:?}", id, self.scoped.bounds(*id));
                }
            }
        }
        let dpi = self.scoped.dpi();
        let gap = GAP.to_px(dpi).value();
        let rows = self.rows.borrow();
        let mut moves = Vec::new();
        for row in rows.iter() {
            if row.items.is_empty() {
                continue;
            }
            let width = self.scoped.bounds(row.node).width();
            if width <= 0 {
                continue;
            }
            let height = row.height.to_px(dpi).value();
            let gaps = gap * (row.items.len() as i32 - 1).max(0);
            let fixed: i32 = row
                .items
                .iter()
                .map(|(_, w)| match w {
                    ItemWidth::Fixed(w) => w.to_px(dpi).value(),
                    ItemWidth::Fill => 0,
                })
                .sum();
            let fill_count = row
                .items
                .iter()
                .filter(|(_, w)| matches!(w, ItemWidth::Fill))
                .count()
                .max(1);
            let fill = ((width - fixed - gaps).max(0)) / fill_count as i32;
            let mut x = 0;
            for (id, spec) in row.items.iter() {
                let item = match spec {
                    ItemWidth::Fixed(w) => w.to_px(dpi).value(),
                    ItemWidth::Fill => fill,
                };
                moves.push((*id, Rect::new(x, 0, x + item, height)));
                x += item + gap;
            }
        }
        self.scoped.apply_moves(&moves);
    }

    /// Records a finished multi-widget row and lays it out.
    fn commit(&self, node: WidgetId, height: Dip, items: Vec<(WidgetId, ItemWidth)>) {
        self.rows.borrow_mut().push(Row {
            node,
            height,
            items,
        });
        self.scroll.add(node, height);
        self.relayout();
    }
}

/// Builds one multi-widget row; call [`RowBuilder::finish`] to register it.
pub struct RowBuilder<'a> {
    page: &'a FormPage,
    node: WidgetId,
    ui: Ui<Msg>,
    height: Dip,
    items: Vec<(WidgetId, ItemWidth)>,
}

impl RowBuilder<'_> {
    /// A [`Ui`] scoped to the row, so its widgets are children of the row.
    pub fn ui(&self) -> &Ui<Msg> {
        &self.ui
    }

    /// Adds `id` as a fixed-width item.
    pub fn fixed(mut self, id: WidgetId, width: Dip) -> Self {
        self.items.push((id, ItemWidth::Fixed(width)));
        self
    }

    /// Adds `id` as an item that fills the row's remaining width.
    pub fn fill(mut self, id: WidgetId) -> Self {
        self.items.push((id, ItemWidth::Fill));
        self
    }

    /// Registers the row with the page.
    pub fn finish(self) {
        self.page.commit(self.node, self.height, self.items);
    }
}
