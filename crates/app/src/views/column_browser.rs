//! The column browser (#112), ported to the portable widget layer: three model
//! `ListView`s (Genre / Artist / Album) above the track table, with cascading
//! selection.
//!
//! All state and logic live in `emusic_ui` ([`ColumnBrowser`]); this module
//! only owns the lists, mirrors the model's facets into them, and turns their
//! selection changes into [`Msg::BrowserRow`]s. Rows are pre-formatted once per
//! refresh so the virtual paint path never allocates.

use std::cell::Cell;

use emusic_ui::views::column_browser::{ColumnBrowser, FacetEntry, Pane, PaneSelection};
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Fill, ListModel, ListView};

use crate::app::Msg;

/// Width of a pane's right-aligned count column, in design units.
const COUNT_WIDTH: f32 = 44.0;

/// One pre-formatted pane row: the label/count text the list draws.
struct FacetRow {
    label: String,
    count: String,
}

impl FacetRow {
    fn new(entry: &FacetEntry) -> Self {
        Self {
            label: entry.label().to_string(),
            count: entry.count.to_string(),
        }
    }
}

/// The model for one pane: the rows in display order.
struct PaneModel {
    rows: Vec<FacetRow>,
}

impl ListModel for PaneModel {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        let row = self.rows.get(row)?;
        Some(if column == 0 {
            row.label.as_str()
        } else {
            row.count.as_str()
        })
    }
}

/// The three cascading panes, laid out side by side.
pub struct ColumnBrowserView {
    ui: Ui<Msg>,
    genre: ListView<Msg>,
    artist: ListView<Msg>,
    album: ListView<Msg>,
    /// The model revision last mirrored into the lists.
    applied_revision: Cell<u64>,
    visible: Cell<bool>,
}

impl ColumnBrowserView {
    /// Creates the three lists and maps their selection changes to
    /// [`Msg::BrowserRow`].
    pub fn new(ui: &Ui<Msg>) -> ColumnBrowserView {
        ColumnBrowserView {
            ui: ui.clone(),
            genre: pane(ui, "Genre", Pane::Genre),
            artist: pane(ui, "Artist", Pane::Artist),
            album: pane(ui, "Album", Pane::Album),
            applied_revision: Cell::new(u64::MAX),
            visible: Cell::new(true),
        }
    }

    /// Moves and sizes the pane strip: the three panes share its width evenly.
    pub fn set_bounds(&self, bounds: Rect) {
        let third = (bounds.right - bounds.left) / 3;
        let rects = [
            Rect::new(bounds.left, bounds.top, bounds.left + third, bounds.bottom),
            Rect::new(
                bounds.left + third,
                bounds.top,
                bounds.left + 2 * third,
                bounds.bottom,
            ),
            Rect::new(
                bounds.left + 2 * third,
                bounds.top,
                bounds.right,
                bounds.bottom,
            ),
        ];
        let moves: Vec<_> = [&self.genre, &self.artist, &self.album]
            .into_iter()
            .zip(rects)
            .map(|(list, rect)| (list.id(), rect))
            .collect();
        self.ui.apply_moves(&moves);
    }

    /// Shows or hides the three panes (the "Column browser" toggle).
    pub fn set_visible(&self, visible: bool) {
        if self.visible.get() == visible {
            return;
        }
        self.visible.set(visible);
        for list in [&self.genre, &self.artist, &self.album] {
            self.ui.set_visible(list.id(), visible);
        }
    }

    /// Mirrors the model's facets and selection into the lists, but only when
    /// the model revision changed.
    pub fn sync(&self, browser: &ColumnBrowser) {
        if self.applied_revision.get() == browser.revision() {
            return;
        }
        self.applied_revision.set(browser.revision());
        self.sync_pane(&self.genre, Pane::Genre, browser);
        self.sync_pane(&self.artist, Pane::Artist, browser);
        self.sync_pane(&self.album, Pane::Album, browser);
    }

    fn sync_pane(&self, list: &ListView<Msg>, pane: Pane, browser: &ColumnBrowser) {
        let entries = browser.facets().pane(pane);
        let selection = selection_of(browser, pane);
        let selected: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| match entry.value.as_deref() {
                None => selection.is_all(),
                Some(value) => selection.contains(value),
            })
            .map(|(index, _)| index)
            .collect();
        list.set_model(PaneModel {
            rows: entries.iter().map(FacetRow::new).collect(),
        });
        list.set_selection(&selected);
    }
}

/// Builds one pane: a virtual list with a left-aligned label and a right-aligned
/// count, whose selection changes map to [`Msg::BrowserRow`] for `pane`.
fn pane(ui: &Ui<Msg>, title: &str, pane: Pane) -> ListView<Msg> {
    ListView::new(ui, Rect::default(), &[])
        .expect("create browser pane")
        .multi_select(true)
        .column(title, Fill)
        .column_right("", dip(COUNT_WIDTH))
        .on_selection(move |rows| {
            Some(Msg::BrowserRow {
                pane,
                rows: rows.to_vec(),
            })
        })
}

/// Reconstructs a pane's selection from the rows a `ListView` reports selected,
/// as a reset followed by one ctrl-click per selected value, so the model's
/// replace/ctrl semantics are preserved without duplicating them here.
///
/// Returns whether the selection changed. The pane sync re-selects rows
/// programmatically (which does not raise an event) and the widget itself only
/// reports genuine user selections, so an unchanged selection never bumps the
/// model revision — which would resync the panes and risk a feedback loop.
pub fn apply_selection(browser: &mut ColumnBrowser, pane: Pane, rows: &[usize]) -> bool {
    use emusic_ui::views::column_browser::ColumnBrowserMsg;

    let values: Vec<String> = rows
        .iter()
        .filter_map(|&index| browser.facets().pane(pane).get(index))
        .filter_map(|entry| entry.value.clone())
        .collect();
    let mut next = PaneSelection::default();
    for value in &values {
        next.click(Some(value), true);
    }
    if next == *selection_of(browser, pane) {
        return false;
    }

    browser.update(ColumnBrowserMsg::RowClicked {
        pane,
        value: None,
        ctrl: false,
    });
    for value in values {
        browser.update(ColumnBrowserMsg::RowClicked {
            pane,
            value: Some(value),
            ctrl: true,
        });
    }
    true
}

/// The selection of `pane` in `browser`.
fn selection_of(browser: &ColumnBrowser, pane: Pane) -> &PaneSelection {
    match pane {
        Pane::Genre => &browser.genres,
        Pane::Artist => &browser.artists,
        Pane::Album => &browser.albums,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emusic_ui::views::column_browser::ColumnBrowserFacets;

    fn browser_with_genres() -> ColumnBrowser {
        let mut browser = ColumnBrowser::default();
        browser.set_facets(ColumnBrowserFacets {
            genres: vec![
                FacetEntry {
                    value: None,
                    count: 3,
                },
                FacetEntry {
                    value: Some("Rock".to_string()),
                    count: 2,
                },
                FacetEntry {
                    value: Some("Jazz".to_string()),
                    count: 1,
                },
            ],
            artists: Vec::new(),
            albums: Vec::new(),
        });
        browser
    }

    #[test]
    fn apply_selection_replaces_then_toggles() {
        let mut browser = browser_with_genres();
        apply_selection(&mut browser, Pane::Genre, &[1]);
        assert!(browser.genres.contains("Rock"));
        assert_eq!(browser.genres.len(), 1);

        // A multi-row selection reconstructs the model's ctrl-click set.
        apply_selection(&mut browser, Pane::Genre, &[1, 2]);
        assert!(browser.genres.contains("Rock"));
        assert!(browser.genres.contains("Jazz"));
        assert_eq!(browser.genres.len(), 2);

        // The "All" row clears the pane.
        apply_selection(&mut browser, Pane::Genre, &[0]);
        assert!(browser.genres.is_all());
    }

    #[test]
    fn apply_selection_reports_whether_it_changed() {
        let mut browser = browser_with_genres();
        // Re-applying the "All" state (what a programmatic re-select echoes
        // back) is not a change, and must not bump the revision.
        let revision = browser.revision();
        assert!(!apply_selection(&mut browser, Pane::Genre, &[0]));
        assert!(!apply_selection(&mut browser, Pane::Genre, &[]));
        assert_eq!(browser.revision(), revision);

        assert!(apply_selection(&mut browser, Pane::Genre, &[1]));
        let revision = browser.revision();
        assert!(!apply_selection(&mut browser, Pane::Genre, &[1]));
        assert_eq!(browser.revision(), revision);
    }
}
