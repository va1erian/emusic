//! The left navigator (#104), ported to the portable [`TreeView`]: the shared
//! view [`SECTIONS`] as collapsible headings, each view a selectable row,
//! marked with a rasterized Lucide icon (#442), that switches the central
//! view.
//!
//! Sections start expanded; collapsing a heading hides its views through the
//! widget's ancestor-visibility filter, and the indented view rows draw indent
//! guides. Clicking a view maps to [`Msg::Navigate`](crate::app::Msg); clicking
//! a heading leaves the active view highlighted.

use std::cell::Cell;
use std::rc::Rc;

use emusic_ui::panels::navigator::SECTIONS;
use emusic_ui::state::View;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::{TreeRow, TreeView};

use super::navigator_icons;
use crate::app::Msg;

/// The navigator's rows, paired with the view each selects (`None` for a
/// section heading), so a click and the highlight resolve through one map.
///
/// Row icons are theme-tinted rasters ([`navigator_icons`]); the rows are
/// rebuilt when the theme flips so the icons stay legible.
fn build_rows(dark: bool) -> (Vec<TreeRow>, Vec<Option<View>>) {
    let mut rows = Vec::new();
    let mut views = Vec::new();
    for section in SECTIONS {
        rows.push(
            TreeRow::new(section.heading, 0)
                .expandable(true)
                .expanded(true),
        );
        views.push(None);
        for &view in section.views {
            rows.push(TreeRow::new(view.label(), 1).icon(navigator_icons::image(view, dark)));
            views.push(Some(view));
        }
    }
    (rows, views)
}

/// The left navigator: the collapsible central-view switcher.
pub struct NavigatorView {
    ui: Ui<Msg>,
    tree: TreeView<Msg>,
    /// For each tree row index, the view it selects (`None` for a section
    /// heading), so a click and the highlight resolve through one map.
    views: Rc<Vec<Option<View>>>,
    /// The active view, so clicking a heading keeps its highlight.
    current: Rc<Cell<View>>,
    /// Whether the rows were built for the dark theme, so [`set_dark`] can
    /// skip a rebuild when the theme is unchanged.
    ///
    /// [`set_dark`]: NavigatorView::set_dark
    dark: Cell<bool>,
}

impl NavigatorView {
    /// Creates the navigator: a collapsible heading per section, then one row
    /// per view.
    pub fn new(ui: &Ui<Msg>) -> NavigatorView {
        let dark = ui.theme().is_dark;
        let (rows, views) = build_rows(dark);

        let views = Rc::new(views);
        let current = Rc::new(Cell::new(View::default()));
        let mapper_views = Rc::clone(&views);
        let mapper_current = Rc::clone(&current);
        let tree = TreeView::new(ui, Rect::default(), &rows)
            .expect("create navigator tree")
            .on_select(move |id| {
                // A heading (no view) keeps the active view selected.
                let view = mapper_views.get(id).copied().flatten();
                Some(Msg::Navigate(view.unwrap_or_else(|| mapper_current.get())))
            });

        NavigatorView {
            ui: ui.clone(),
            tree,
            views,
            current,
            dark: Cell::new(dark),
        }
    }

    /// Re-tints the row icons for the light or dark theme, rebuilding the rows
    /// only when the theme actually changed. An image paints in its own
    /// colours, so unlike a vector glyph it cannot follow the row's text
    /// colour and carries a themed raster instead.
    pub fn set_dark(&self, dark: bool) {
        if self.dark.get() == dark {
            return;
        }
        self.dark.set(dark);
        let (rows, _) = build_rows(dark);
        self.tree.set_rows(&rows);
    }

    /// Moves/resizes the navigator.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.tree.id(), rect)]);
    }

    /// Shows or hides the navigator.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.tree.id(), visible);
    }

    /// Selects the row for the active view without raising an event.
    pub fn sync(&self, view: View) {
        self.current.set(view);
        let index = self.views.iter().position(|entry| *entry == Some(view));
        self.tree.select(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xui::xui_core::widget::RowIcon;

    /// The navigator's row map, without a window: headings map to `None`, view
    /// rows to their view, and every view appears exactly once.
    fn row_map() -> Vec<Option<View>> {
        build_rows(false).1
    }

    #[test]
    fn every_view_has_exactly_one_row() {
        let listed: Vec<View> = row_map().into_iter().flatten().collect();
        for &view in &listed {
            assert_eq!(
                listed.iter().filter(|&&other| other == view).count(),
                1,
                "{view:?} appears more than once"
            );
        }
        assert_eq!(listed.len(), View::ALL.len() - 1, "Settings is menu-only");
    }

    #[test]
    fn headings_are_non_selectable_rows() {
        let map = row_map();
        let heading_rows = map.iter().filter(|entry| entry.is_none()).count();
        assert_eq!(heading_rows, SECTIONS.len(), "one heading per section");
        assert_eq!(
            map.first().copied().flatten(),
            None,
            "a section heading is the first row"
        );
        assert_eq!(map.get(1).copied().flatten(), Some(View::Music));
    }

    #[test]
    fn every_view_row_carries_a_rasterized_icon() {
        // Headings are plain, every view row an `Image` (not a `Glyph`), so a
        // regression back to the hand-drawn set fails here.
        let (rows, views) = build_rows(false);
        for (row, view) in rows.iter().zip(&views) {
            match view {
                Some(view) => assert!(
                    matches!(row.icon, Some(RowIcon::Image(_))),
                    "{view:?} carries a rasterized icon"
                ),
                None => assert!(row.icon.is_none(), "a heading has no icon"),
            }
        }
    }
}
