//! Win32 column browser (#112): three virtual (owner-data) `ListView`s
//! (Genre / Artist / Album) above the track table, with cascading selection.
//!
//! All state and logic live in `emusic_ui` ([`ColumnBrowser`]); this module
//! only owns the native lists, mirrors the model's facets into them, and turns
//! their selection changes into [`Msg::BrowserRow`]s. Rows are pre-formatted
//! once per refresh so owner-data requests never allocate.

use std::cell::Cell;
use std::rc::Rc;

use emusic_ui::views::column_browser::{ColumnBrowser, FacetEntry, Pane, PaneSelection};
use win32ui::prelude::*;
use win32ui::{ColumnWidth, Fill, Layout, ListModel, ListView, dip, row};

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

/// The owner-data model for one pane.
struct PaneModel {
    rows: Vec<FacetRow>,
}

impl ListModel for PaneModel {
    type Item = FacetRow;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> Option<&FacetRow> {
        self.rows.as_slice().get(index)
    }
}

/// The three cascading panes, laid out side by side.
pub struct ColumnBrowserView {
    genre: ListView<FacetRow, Msg>,
    artist: ListView<FacetRow, Msg>,
    album: ListView<FacetRow, Msg>,
    /// The model revision last mirrored into the lists.
    applied_revision: Cell<u64>,
    visible: Cell<bool>,
    /// Set while [`Self::sync`] rewrites the lists, so the selection changes it
    /// causes are not reported back as user clicks.
    syncing: Rc<Cell<bool>>,
}

impl ColumnBrowserView {
    /// Creates the three owner-data lists and maps their selection changes to
    /// [`Msg::BrowserRow`].
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        let syncing = Rc::new(Cell::new(false));
        Ok(Self {
            genre: pane(ui, "Genre", Pane::Genre, &syncing)?,
            artist: pane(ui, "Artist", Pane::Artist, &syncing)?,
            album: pane(ui, "Album", Pane::Album, &syncing)?,
            applied_revision: Cell::new(u64::MAX),
            visible: Cell::new(true),
            syncing,
        })
    }

    /// Mirrors the model's facets and selection into the lists, but only when
    /// the model revision changed.
    pub fn sync(&self, browser: &ColumnBrowser) {
        if self.applied_revision.get() == browser.revision() {
            return;
        }
        self.applied_revision.set(browser.revision());
        self.syncing.set(true);
        self.sync_pane(&self.genre, Pane::Genre, browser);
        self.sync_pane(&self.artist, Pane::Artist, browser);
        self.sync_pane(&self.album, Pane::Album, browser);
        self.syncing.set(false);
    }

    /// Shows or hides the three panes (the menu's "Column browser" toggle).
    pub fn set_visible(&self, visible: bool) {
        if self.visible.get() == visible {
            return;
        }
        self.visible.set(visible);
        self.genre.set_visible(visible);
        self.artist.set_visible(visible);
        self.album.set_visible(visible);
    }

    /// The pane strip, meant to sit above the track table. Hidden panes take no
    /// space, so the strip collapses when the browser is toggled off.
    pub fn layout(&self) -> Layout {
        row![self.genre.fill(1), self.artist.fill(1), self.album.fill(1)]
    }

    fn sync_pane(&self, list: &ListView<FacetRow, Msg>, pane: Pane, browser: &ColumnBrowser) {
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
/// count, whose selection changes map to [`Msg::BrowserRow`] for `pane`
/// (except while `syncing`, when the model itself is being mirrored in).
fn pane(
    ui: &mut Ui<Msg>,
    title: &str,
    pane: Pane,
    syncing: &Rc<Cell<bool>>,
) -> Result<ListView<FacetRow, Msg>> {
    let syncing = Rc::clone(syncing);
    Ok(ListView::new(ui)?
        .multi_select(true)
        .column(title, Fill, |row: &FacetRow| row.label.as_str())
        .column_right(
            "",
            ColumnWidth::Fixed(dip(COUNT_WIDTH)),
            |row: &FacetRow| row.count.as_str(),
        )
        .on_select(move |rows| {
            (!syncing.get()).then(|| Msg::BrowserRow {
                pane,
                rows: rows.to_vec(),
            })
        }))
}

/// Reconstructs a pane's selection from the rows a `ListView` reports selected,
/// as a reset followed by one ctrl-click per selected value, so the model's
/// replace/ctrl semantics are preserved without duplicating them here.
///
/// Returns whether the selection changed. The pane sync re-selects rows
/// programmatically, which reports the selection back as a [`Msg::BrowserRow`];
/// an unchanged selection must not touch the model (bumping its revision
/// resyncs the panes and re-emits, looping forever on the UI thread).
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
    use emusic_ui::views::column_browser::{ColumnBrowserFacets, FacetEntry};

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
