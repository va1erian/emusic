//! A scrollable Settings form page on the portable [`ScrollView`].
//!
//! `xui_core`'s `ScrollView` stacks one widget per full-width row; a form needs
//! "label | field" rows too, so [`FormPage`] adds two kinds of row:
//!
//! - [`FormPage::add_full`] registers a single widget as a full-width row
//!   (headings, checkboxes, a [`RadioGroup`] option, a [`ListView`]).
//! - [`FormPage::row`] registers a row-height spacer and lays several widgets
//!   out over it left to right, each either a fixed width or filling the
//!   remainder ([`RowBuilder::fixed`] / [`RowBuilder::fill`]).
//!
//! A row's widgets are parented to the *scroll view*, not to the row container,
//! as siblings of it: joining a painted child window to a painted parent stops
//! the Win32 backend painting them (the same reason xui's own `Dialog` keeps its
//! controls beside the scrim rather than inside it). The container is only a
//! spacer that gives the scroll view its height, and [`position_items`] places
//! the widgets over it from the container's current (scrolled) bounds. The
//! `on_scroll` mapping re-runs that placement so the widgets follow the scroll.
//!
//! The scroll extent is the sum of the row heights, so the scrollbar matches
//! exactly. Re-laying the widgets after the host has positioned a row keeps the
//! form correct on resize and on the first layout after a tab becomes active.

use std::cell::RefCell;
use std::rc::Rc;

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

/// One registered row: the spacer node the host scrolls, and the widgets to
/// place over it (empty for a full-width single-widget row, whose node is the
/// widget itself).
struct Row {
    node: WidgetId,
    height: Dip,
    items: Vec<(WidgetId, ItemWidth)>,
}

/// A page's form: the scroll host plus its registered rows.
pub struct FormPage {
    scroll: ScrollView<Msg>,
    scoped: Ui<Msg>,
    rows: Rc<RefCell<Vec<Row>>>,
}

impl FormPage {
    /// Creates an empty scrollable form page.
    pub fn new(ui: &Ui<Msg>) -> Result<FormPage> {
        let scroll = ScrollView::new(ui, Rect::default())?;
        let scoped = scroll.ui().clone();
        let rows: Rc<RefCell<Vec<Row>>> = Rc::new(RefCell::new(Vec::new()));
        // Scrolling moves the row containers; re-place the widgets over them so
        // they move with the scroll instead of staying put.
        let on_scroll_rows = Rc::clone(&rows);
        let on_scroll_ui = scoped.clone();
        let scroll_id = scroll.id();
        let scroll = scroll.on_scroll(move |_offset| {
            position_items(&on_scroll_ui, scroll_id, &on_scroll_rows.borrow());
            None
        });
        Ok(FormPage {
            scroll,
            scoped,
            rows,
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
        // The spacer paints the row background so the widgets over it (and the
        // scroll view's own surface) never show the uninitialised back buffer.
        let theme = self.scoped.theme_handle();
        self.scoped.set_painter(
            node,
            Rc::new(move |canvas| canvas.clear(theme.get().background)),
        );
        // The widgets are siblings of the spacer, parented to the scroll view,
        // so the Win32 backend paints them (see the module docs).
        let ui = self.scoped.with_parent(self.scroll.id());
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
    ///
    /// The row widgets are siblings of the scroll view, so a backend that does
    /// not cascade a hidden parent's visibility needs them hidden explicitly
    /// (they are shown again the next time [`Self::relayout`] places them).
    pub fn set_visible(&self, visible: bool) {
        self.scroll.set_visible(visible);
        if !visible {
            for row in self.rows.borrow().iter() {
                for (id, _) in &row.items {
                    self.scoped.set_visible(*id, false);
                }
            }
        }
    }

    /// Re-lays the scroll content and then places every row's widgets.
    pub fn relayout(&self) {
        self.scroll.relayout();
        position_items(&self.scoped, self.scroll.id(), &self.rows.borrow());
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

/// Places every multi-widget row's widgets over its container, using the
/// container's current (scrolled) bounds.
fn position_items(ui: &Ui<Msg>, scroll_id: WidgetId, rows: &[Row]) {
    let dpi = ui.dpi();
    let gap = GAP.to_px(dpi).value();
    // Row containers are placed in the scroll view's own coordinates (origin at
    // its top-left), so the viewport is `0..height`, not the node's parent-space
    // bounds.
    let viewport_height = ui.bounds(scroll_id).height();
    let mut moves = Vec::new();
    let mut hidden = Vec::new();
    for row in rows {
        if row.items.is_empty() {
            continue;
        }
        let container = ui.bounds(row.node);
        let width = container.width();
        if width <= 0 {
            continue;
        }
        // The host culls a row scrolled out of the viewport; hide its widgets
        // too, since they are siblings of the culled spacer rather than
        // children of it.
        let on_screen = container.bottom > 0 && container.top < viewport_height;
        if !on_screen {
            hidden.extend(row.items.iter().map(|(id, _)| *id));
            continue;
        }
        let top = container.top;
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
            ui.set_visible(*id, true);
            // Clamp to the viewport so a too-wide row cannot cover the
            // scrollbar in the band the view reserves for it.
            let right = (x + item).min(width);
            moves.push((*id, Rect::new(x, top, right, top + height)));
            x += item + gap;
        }
    }
    ui.apply_moves(&moves);
    // A moved window is not repainted by the window manager and a later sibling
    // may cover it, so raise and repaint each widget at its new bounds.
    for (id, _) in moves.iter() {
        ui.raise(*id);
        ui.invalidate(*id);
    }
    for id in hidden {
        ui.set_visible(id, false);
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
    /// A [`Ui`] scoped to the row, so its widgets live over the row container.
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
