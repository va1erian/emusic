//! The scrollable host for a Settings page (xui #119).
//!
//! A page's controls are created through a `xui` [`Panel`]'s scoped `Ui`,
//! so they parent to the panel rather than the top-level window; the panel is
//! handed to a [`ScrollView`], so the whole form scrolls with the wheel and the
//! scrollbar when a page is taller than the window.

use xui::Dip;
use xui::prelude::*;

use crate::app::Msg;

/// Gap between the rows of a page's form, in design units.
const ROW_SPACING: f32 = 8.0;
/// Padding inside a page's scrolling panel, in design units.
const PAGE_MARGIN: f32 = 12.0;

/// One row of a page's scrollable form: its layout item and the design-unit
/// height it occupies, so the scroll extent can be summed exactly.
pub(super) type FormRow = (LayoutItem, f32);

/// Builds a page's form column and the exact content height it needs, so the
/// hosting [`ScrollView`] sizes its scrollbar to the rows.
fn build_form(rows: Vec<FormRow>) -> (Layout, Dip) {
    let mut column = Layout::column()
        .spacing(dip(ROW_SPACING))
        .margins(Insets::all(dip(PAGE_MARGIN)));
    let mut height = dip(2.0 * PAGE_MARGIN);
    for (index, (item, row)) in rows.into_iter().enumerate() {
        if index > 0 {
            height = height + dip(ROW_SPACING);
        }
        height = height + dip(row);
        column = column.item(item);
    }
    (column, height)
}

/// A page's controls hosted in a `xui` [`Panel`] inside a [`ScrollView`].
pub(super) struct ScrollPanel {
    view: ScrollView,
    panel: Panel,
}

impl ScrollPanel {
    /// Creates the scroll view and installs an empty content panel.
    pub(super) fn new(ui: &mut Ui<Msg>) -> xui::Result<Self> {
        let view = ScrollView::new(ui)?;
        let panel = Panel::new(ui)?;
        view.set_content(&panel);
        Ok(Self { view, panel })
    }

    /// A [`Ui`] scoped to the panel: controls created through it parent to the
    /// panel and scroll with it.
    pub(super) fn ui(&self, ui: &Ui<Msg>) -> Ui<Msg> {
        self.panel.ui(ui)
    }

    /// Installs `rows` as the panel's form and sizes the scroll extent to it.
    /// Call again after a row's visibility changes.
    pub(super) fn apply(&self, ui: &Ui<Msg>, rows: Vec<FormRow>) {
        let (layout, height) = build_form(rows);
        self.panel.set_layout(layout);
        self.view.set_content_height(height.to_px(ui.dpi()));
        self.panel.relayout();
    }

    /// The scroll view as one page of the tab strip.
    pub(super) fn page(&self) -> LayoutItem {
        self.view.fill(1)
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.view.set_visible(visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_height_adds_margins_spacing_and_rows() {
        let (_layout, height) =
            build_form(vec![(dummy(), 24.0), (dummy(), 28.0), (dummy(), 180.0)]);
        assert_eq!(
            height.value(),
            2.0 * PAGE_MARGIN + 24.0 + 28.0 + 180.0 + 2.0 * ROW_SPACING
        );
    }

    #[test]
    fn content_height_of_an_empty_form_is_just_the_margins() {
        let (_layout, height) = build_form(Vec::new());
        assert_eq!(height.value(), 2.0 * PAGE_MARGIN);
    }

    /// An empty nested layout stands in for a real row: the height maths never
    /// touches the item.
    fn dummy() -> LayoutItem {
        Layout::row().into_layout_item()
    }
}
