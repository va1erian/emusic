//! A scrollable Settings form page on the portable [`ScrollView`].
//!
//! A form needs "label | field" rows as well as full-width ones, so
//! [`FormPage`] keeps its own list of rows:
//!
//! - [`FormPage::add_full`] registers a single widget as a full-width row
//!   (headings, checkboxes, a [`RadioGroup`] option, a [`ListView`]).
//! - [`FormPage::row`] lays several widgets out left to right, each either a
//!   fixed width or filling the remainder ([`RowBuilder::fixed`] /
//!   [`RowBuilder::fill`]).
//!
//! The scroll view's content is one [`FormContent`] leaf: it measures as the
//! sum of the row heights, so the scrollbar matches exactly, and when the view
//! places it (on resize and at every scroll offset) it places each row's
//! widgets over itself. The widgets are its siblings, children of the scroll
//! view, and a row scrolled out of the viewport has its widgets hidden.
//!
//! [`RadioGroup`]: xui::xui_core::widget::RadioGroup
//! [`ListView`]: xui::xui_core::widget::ListView

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use xui::xui_core::app::Ui;
use xui::xui_core::arrange::{LayoutExt, build, column, scroll};
use xui::xui_core::backend::{NodeKind, NodeSpec, Result, WidgetId};
use xui::xui_core::geometry::{Rect, Size};
use xui::xui_core::layout::Constraints;
use xui::xui_core::units::{Dip, dip};
use xui::xui_core::widget::{Control, Placeable, ScrollView};

use crate::app::Msg;
use crate::make::make;

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

/// One registered row: its height and the widgets placed across it.
struct Row {
    height: Dip,
    items: Vec<(WidgetId, ItemWidth)>,
}

/// The rows a page shares with its [`FormContent`].
type Rows = Rc<RefCell<Vec<Row>>>;

/// A page's form: the scroll host plus its registered rows.
pub struct FormPage {
    scroll: ScrollView<Msg>,
    scoped: Ui<Msg>,
    rows: Rows,
}

impl FormPage {
    /// Creates an empty scrollable form page.
    pub fn new(ui: &Ui<Msg>) -> Result<FormPage> {
        let rows: Rows = Rc::new(RefCell::new(Vec::new()));
        // The content is created inside the view, before the view's id is
        // known to the page; it reads it from here once it is placed.
        let scroll_id = Rc::new(Cell::new(WidgetId::NONE));
        let content = {
            let rows = Rc::clone(&rows);
            let scroll_id = Rc::clone(&scroll_id);
            build(move |ui: &Ui<Msg>| FormContent::new(ui, rows, scroll_id))
        };
        let scroll = make(ui, scroll(column().child(content.fill(1))))?;
        scroll_id.set(scroll.id());
        let scoped = scroll.ui().clone();
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
        self.commit(height, vec![(id, ItemWidth::Fill)]);
    }

    /// Starts a multi-widget row of `height`; finish it with
    /// [`RowBuilder::finish`].
    pub fn row(&self, height: Dip) -> Result<RowBuilder<'_>> {
        Ok(RowBuilder {
            page: self,
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
    /// The row widgets are siblings of the content node, so a backend that
    /// does not cascade a hidden parent's visibility needs them hidden
    /// explicitly (they are shown again the next time the view places them).
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

    /// Re-measures the rows and places them at the current scroll offset.
    pub fn relayout(&self) {
        self.scroll.relayout();
    }

    /// Records a finished row and lays the page out again.
    fn commit(&self, height: Dip, items: Vec<(WidgetId, ItemWidth)>) {
        self.rows.borrow_mut().push(Row { height, items });
        self.relayout();
    }
}

/// The scroll view's content: a background node as tall as the rows, which
/// places the row widgets whenever the view places it.
struct FormContent {
    node: Control<Msg>,
    scroll: Rc<Cell<WidgetId>>,
    rows: Rows,
}

impl FormContent {
    fn new(ui: &Ui<Msg>, rows: Rows, scroll: Rc<Cell<WidgetId>>) -> Result<FormContent> {
        let node = Control::new(ui, &NodeSpec::new(NodeKind::Container, Rect::default()))?;
        // The content paints the page background so the gaps between the row
        // widgets never show the uninitialised back buffer.
        let theme = ui.theme_handle();
        node.set_painter(Rc::new(move |canvas| canvas.clear(theme.get().background)));
        Ok(FormContent { node, scroll, rows })
    }
}

impl Placeable<Msg> for FormContent {
    fn id(&self) -> WidgetId {
        self.node.id()
    }

    fn measure(&self, _ui: &Ui<Msg>, constraints: Constraints) -> Size {
        let height = self
            .rows
            .borrow()
            .iter()
            .map(|row| row.height.to_px(constraints.dpi).value())
            .sum();
        Size::new(constraints.max_width.unwrap_or(0), height)
    }

    fn placed(&self, ui: &Ui<Msg>, rect: Rect) {
        let scroll = self.scroll.get();
        if scroll.is_none() {
            return;
        }
        let viewport_height = ui.bounds(scroll).height();
        place_rows(ui, rect, viewport_height, &self.rows.borrow());
    }
}

/// Places every row's widgets down `content` (the content's rect in the
/// scroll view's coordinates, shifted by the scroll offset), hiding the rows
/// scrolled out of the `0..viewport_height` band.
fn place_rows(ui: &Ui<Msg>, content: Rect, viewport_height: i32, rows: &[Row]) {
    let dpi = ui.dpi();
    let gap = GAP.to_px(dpi).value();
    let width = content.width();
    let mut moves = Vec::new();
    let mut top = content.top;
    for row in rows {
        let height = row.height.to_px(dpi).value();
        let on_screen = width > 0 && top + height > 0 && top < viewport_height;
        if !on_screen {
            for (id, _) in &row.items {
                show(ui, *id, false);
            }
            top += height;
            continue;
        }
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
        let mut x = content.left;
        for (id, spec) in row.items.iter() {
            let item = match spec {
                ItemWidth::Fixed(w) => w.to_px(dpi).value(),
                ItemWidth::Fill => fill,
            };
            show(ui, *id, true);
            // Clamp to the content so a too-wide row cannot cover the
            // scrollbar in the band the view reserves for it.
            let right = (x + item).min(content.right);
            moves.push((*id, Rect::new(x, top, right, top + height)));
            x += item + gap;
        }
        top += height;
    }
    ui.apply_moves(&moves);
}

/// Shows or hides `id`, touching it only on a change: a visibility change
/// re-flows the window's layouts, and this runs inside one.
fn show(ui: &Ui<Msg>, id: WidgetId, visible: bool) {
    if ui.is_visible(id) != visible {
        ui.set_visible(id, visible);
    }
}

/// Builds one multi-widget row; call [`RowBuilder::finish`] to register it.
pub struct RowBuilder<'a> {
    page: &'a FormPage,
    height: Dip,
    items: Vec<(WidgetId, ItemWidth)>,
}

impl RowBuilder<'_> {
    /// The [`Ui`] the row's widgets are created through: the page's.
    pub fn ui(&self) -> &Ui<Msg> {
        self.page.ui()
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
        self.page.commit(self.height, self.items);
    }
}
