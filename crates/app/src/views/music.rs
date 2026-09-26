//! The Music view (#107), ported to the portable widget layer: the shared track
//! table over the library, filtered by the column browser and the top-bar
//! search.
//!
//! All formatting and ordering comes from `emusic-ui`; this module only builds
//! the filtered track slice and hands it to the reusable [`TrackView`]. The
//! header row (track count + "Shuffle all", #242) sits above the table.

use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::search::SearchEngine;
use emusic_ui::shell::Changes;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::column_browser::ColumnBrowser;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, HasText, Label};

use crate::app::Msg;
use crate::views::track_table::TrackView;

/// Header row height, in device-independent pixels.
const HEADER_HEIGHT: f32 = 28.0;
/// Width of the "Shuffle all" button, in device-independent pixels.
const SHUFFLE_BUTTON_WIDTH: f32 = 110.0;
/// Horizontal inset of the header contents, in device-independent pixels.
const HEADER_INSET: f32 = 8.0;

/// What the model was built from; the table is only rebuilt when it changes.
#[derive(PartialEq)]
struct Signature {
    track_count: usize,
    scan: bool,
    search_active: bool,
    search_count: Option<usize>,
    column_browser: ColumnBrowser,
}

/// The Music view: the library track table filtered by the browser and search.
pub struct MusicView {
    ui: Ui<Msg>,
    count: Label<Msg>,
    shuffle: Button<Msg>,
    table: TrackView,
    signature: Option<Signature>,
}

impl MusicView {
    /// Creates the view, its header row and its (empty) virtual list.
    pub fn new(ui: &Ui<Msg>) -> MusicView {
        let count = Label::new(ui, Rect::default(), "0 tracks").expect("create track count");
        let shuffle = Button::new(ui, Rect::default(), "Shuffle all")
            .expect("create shuffle button")
            .on_click(|| Some(Msg::MusicShuffleAll));
        MusicView {
            ui: ui.clone(),
            count,
            shuffle,
            table: TrackView::new(ui),
            signature: None,
        }
    }

    /// Moves and sizes the whole view inside `bounds`: the header row on top,
    /// the table filling the rest.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let header = dip(HEADER_HEIGHT).to_px(dpi).value();
        let inset = dip(HEADER_INSET).to_px(dpi).value();
        let button_width = dip(SHUFFLE_BUTTON_WIDTH).to_px(dpi).value();
        self.ui.apply_moves(&[
            (
                self.count.id(),
                Rect::new(
                    bounds.left + inset,
                    bounds.top,
                    bounds.right - button_width - inset,
                    bounds.top + header,
                ),
            ),
            (
                self.shuffle.id(),
                Rect::new(
                    bounds.right - button_width,
                    bounds.top,
                    bounds.right - inset,
                    bounds.top + header,
                ),
            ),
        ]);
        self.table.set_bounds(Rect::new(
            bounds.left,
            bounds.top + header,
            bounds.right,
            bounds.bottom,
        ));
    }

    /// Shows or hides the header row and the table together.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.count.id(), visible);
        self.ui.set_visible(self.shuffle.id(), visible);
        self.table.set_visible(visible);
    }

    /// The command to shuffle-play the tracks currently visible in the table
    /// (the "Shuffle all" button, #242).
    pub fn shuffle_all(
        &self,
        state: &AppState,
        library: &dyn LibraryDataSource,
        search: &SearchEngine,
    ) -> Command {
        let ids: Vec<u64> = visible_tracks(library, search, &state.music.browser)
            .iter()
            .map(|track| track.id)
            .collect();
        Command::ShuffleScope {
            ids,
            label: "Music".to_string(),
        }
    }

    /// Rebuilds the model and playing highlight from the shell state. `changed`
    /// gates the expensive row rebuild.
    pub fn sync(
        &mut self,
        state: &AppState,
        library: &dyn LibraryDataSource,
        search: &SearchEngine,
        playing_id: Option<u64>,
        changed: Changes,
    ) {
        let signature = Signature {
            track_count: library.track_count(),
            scan: library.is_scanning(),
            search_active: search.is_active(),
            search_count: search.match_count(),
            column_browser: state.music.browser.clone(),
        };
        let stale = self.signature.as_ref() != Some(&signature);
        if stale {
            let browser_changed = self
                .signature
                .as_ref()
                .is_none_or(|previous| previous.column_browser != signature.column_browser);
            if changed.intersects(Changes::LIBRARY | Changes::SEARCH) || browser_changed {
                let tracks = visible_tracks(library, search, &signature.column_browser);
                self.count.set_text(&format!("{} tracks", tracks.len()));
                self.table.set_rows(&tracks, state.music.table.sort);
                self.signature = Some(signature);
            }
        }

        self.table.sync_playing(playing_id);
    }

    /// Applies the current sort order to the model (after a header click).
    pub fn resort(
        &mut self,
        state: &AppState,
        library: &dyn LibraryDataSource,
        search: &SearchEngine,
    ) {
        let tracks = visible_tracks(library, search, &state.music.browser);
        self.table.set_rows(&tracks, state.music.table.sort);
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<Command> {
        self.table.activate(index)
    }

    /// The command to toggle the star of `index`.
    pub fn toggle_star(&self, index: usize) -> Option<Command> {
        self.table.toggle_star(index)
    }

    /// The track at `index`, for a context action.
    pub fn track(&self, index: usize) -> Option<TrackInfo> {
        self.table.track(index).cloned()
    }
}

/// The library tracks the Music view shows: those passing the column browser
/// and the live search.
fn visible_tracks<'a>(
    library: &'a dyn LibraryDataSource,
    search: &SearchEngine,
    browser: &ColumnBrowser,
) -> Vec<&'a TrackInfo> {
    library
        .tracks()
        .iter()
        .filter(|track| browser.matches(track))
        .filter(|track| search.is_match(track.id))
        .collect()
}
