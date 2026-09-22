//! MusicBee-style column browser (#16): three cascading filter panes
//! (Genre | Artist | Album) shown above the Music view's track table.
//!
//! Each pane is a virtualized list whose first row is "All (N)". Selecting a
//! facet narrows the panes to its right and the tracks below; a pane with
//! nothing selected behaves as "All". Ctrl-click multi-selects. The splitter
//! height and visibility are persisted through [`crate::config::Config`].

mod pane;
mod selection;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};

use eframe::egui;

use crate::library_api::{LibraryDataSource, TrackInfo};

use pane::PaneEntry;
pub use selection::PaneSelection;

/// Default splitter height, in pixels (the pane strip above the table).
pub const DEFAULT_HEIGHT: f32 = 150.0;
/// Smallest the column browser may be dragged to.
pub const MIN_HEIGHT: f32 = 60.0;
/// Largest the column browser may be dragged to.
pub const MAX_HEIGHT: f32 = 480.0;

/// Persistent column-browser state: visibility, splitter height and the
/// selection of each pane.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnBrowserState {
    pub visible: bool,
    pub height: f32,
    pub genres: PaneSelection,
    pub artists: PaneSelection,
    pub albums: PaneSelection,
}

impl Default for ColumnBrowserState {
    fn default() -> Self {
        Self {
            visible: true,
            height: DEFAULT_HEIGHT,
            genres: PaneSelection::default(),
            artists: PaneSelection::default(),
            albums: PaneSelection::default(),
        }
    }
}

impl ColumnBrowserState {
    /// Whether `track` passes all three panes' filters.
    pub fn matches(&self, track: &TrackInfo) -> bool {
        self.genres.matches(&track.genre)
            && self.artists.matches(&track.artist)
            && self.albums.matches(&track.album)
    }
}

/// Renders the three panes in a resizable top panel, cascading the
/// selections left to right and pruning child selections that the parent no
/// longer offers. Must run before filtering the track table, so the table
/// sees the same selections this frame.
pub fn show(ui: &mut egui::Ui, state: &mut ColumnBrowserState, library: &dyn LibraryDataSource) {
    let all_tracks: Vec<&TrackInfo> = library.tracks().iter().collect();

    // Genre is the root: options come from the whole library.
    let genres = entries_for(&all_tracks, |t| &t.genre);
    state.genres.retain(&values(&genres));

    let by_genre: Vec<&TrackInfo> = all_tracks
        .iter()
        .copied()
        .filter(|t| state.genres.matches(&t.genre))
        .collect();
    let artists = entries_for(&by_genre, |t| &t.artist);
    state.artists.retain(&values(&artists));

    let by_artist: Vec<&TrackInfo> = by_genre
        .iter()
        .copied()
        .filter(|t| state.artists.matches(&t.artist))
        .collect();
    let albums = entries_for(&by_artist, |t| &t.album);
    state.albums.retain(&values(&albums));

    let response = egui::Panel::top("column_browser")
        .resizable(true)
        .default_size(state.height)
        .min_size(MIN_HEIGHT)
        .max_size(MAX_HEIGHT)
        .show(ui, |ui| {
            ui.columns(3, |columns| {
                pane::show(
                    &mut columns[0],
                    "column_browser_genre",
                    "Genre",
                    &genres,
                    &mut state.genres,
                );
                pane::show(
                    &mut columns[1],
                    "column_browser_artist",
                    "Artist",
                    &artists,
                    &mut state.artists,
                );
                pane::show(
                    &mut columns[2],
                    "column_browser_album",
                    "Album",
                    &albums,
                    &mut state.albums,
                );
            });
        });

    // Mirror the panel's live size (which follows a user drag) back into the
    // state so the config can persist it.
    state.height = response.response.rect.height();
}

/// Builds a pane's rows: "All (N)" first, then each distinct facet sorted
/// case-insensitively, with the number of tracks it represents.
fn entries_for(tracks: &[&TrackInfo], key: impl Fn(&TrackInfo) -> &str) -> Vec<PaneEntry> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for track in tracks.iter().copied() {
        *counts.entry(key(track).to_string()).or_default() += 1;
    }

    let mut facets: Vec<(String, usize)> = counts.into_iter().collect();
    facets.sort_by(|a, b| {
        a.0.to_lowercase()
            .cmp(&b.0.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut entries = Vec::with_capacity(facets.len() + 1);
    entries.push(PaneEntry {
        value: None,
        count: tracks.len(),
    });
    entries.extend(facets.into_iter().map(|(value, count)| PaneEntry {
        value: Some(value),
        count,
    }));
    entries
}

fn values(entries: &[PaneEntry]) -> BTreeSet<String> {
    entries.iter().filter_map(|e| e.value.clone()).collect()
}
