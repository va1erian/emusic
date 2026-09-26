//! Win32 Music view (#107): the shared track table over the library, filtered
//! by the column browser and the top-bar search.
//!
//! All formatting and ordering comes from `emusic-ui`; this module only builds
//! the filtered track slice and hands it to the reusable [`TrackView`]. The
//! header row (track count + "Shuffle all", #242) sits above the table.

use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::search::SearchEngine;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::column_browser::ColumnBrowser;
use xui::prelude::*;
use xui::{Button, Control, Label, LayoutItem, Menu, Rect, Result, Ui, dip, row};

use crate::app::Msg;
use crate::views::track_table::TrackView;

/// Header row height, in device-independent pixels.
const HEADER_HEIGHT: f32 = 28.0;
/// Width of the "Shuffle all" button, in device-independent pixels.
const SHUFFLE_BUTTON_WIDTH: f32 = 100.0;

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
    count_label: Label,
    shuffle_button: Button<Msg>,
    table: TrackView,
    signature: Option<Signature>,
}

impl MusicView {
    /// Creates the view, its header row and its (empty) virtual list.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        let count_label = Label::new(ui, Rect::default(), "0 tracks")?;
        let shuffle_button =
            Button::new(ui, "Shuffle all")?.on_click(|| Some(Msg::MusicShuffleAll));
        Ok(Self {
            count_label,
            shuffle_button,
            table: TrackView::new(ui)?,
            signature: None,
        })
    }

    /// The header row: the visible track count and the "Shuffle all" button.
    pub fn header(&self) -> LayoutItem {
        row![
            self.count_label.fill(1),
            self.shuffle_button.width(dip(SHUFFLE_BUTTON_WIDTH)),
        ]
        .height(dip(HEADER_HEIGHT))
    }

    /// Shows or hides the header row and the table together.
    pub fn set_visible(&self, visible: bool) {
        self.count_label.set_visible(visible);
        self.shuffle_button.set_visible(visible);
        self.table.set_visible(visible);
    }

    /// Applies the current appearance metrics and zebra flag (#309).
    pub fn apply_appearance(&self) {
        self.table.apply_appearance();
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
        library: &dyn emusic_ui::library_api::LibraryDataSource,
        search: &SearchEngine,
        playing_id: Option<u64>,
        changed: emusic_ui::shell::Changes,
    ) {
        use emusic_ui::shell::Changes;

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
                self.count_label
                    .set_text(&format!("{} tracks", tracks.len()));
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
        library: &dyn emusic_ui::library_api::LibraryDataSource,
        search: &SearchEngine,
    ) {
        let tracks = visible_tracks(library, search, &state.music.browser);
        self.table.set_rows(&tracks, state.music.table.sort);
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<emusic_ui::state::Command> {
        self.table.activate(index)
    }

    /// The command to toggle the star of `index` (a star-cell click).
    pub fn toggle_star(&self, index: usize) -> Option<emusic_ui::state::Command> {
        self.table.toggle_star(index)
    }

    /// Runs a context action on the row that opened the menu.
    pub fn run_context(
        &self,
        action: crate::views::track_table::ContextAction,
        hwnd: xui::Hwnd,
    ) -> Option<emusic_ui::state::Command> {
        self.table.run_context(action, hwnd)
    }

    pub fn set_context_row(&self, row: usize) {
        self.table.set_context_row(row);
    }

    /// The track whose context menu is open, if any.
    pub fn context_track(&self) -> Option<emusic_ui::library_api::TrackInfo> {
        self.table.context_track()
    }

    pub fn context_menu(&self) -> &Menu<Msg> {
        self.table.context_menu()
    }
}

impl AsControl for MusicView {
    fn control(&self) -> &Control {
        self.table.control()
    }
}

/// The library tracks the Music view shows: those passing the column browser
/// and the live search.
fn visible_tracks<'a>(
    library: &'a dyn emusic_ui::library_api::LibraryDataSource,
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
