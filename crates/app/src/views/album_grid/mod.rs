//! "Albums" view (#17): a virtualized grid of cover tiles, backed by an
//! off-thread thumbnail cache, with the selected album's tracks shown in the
//! shared track table below.
//!
//! The grid is virtualized by `ScrollArea::show_rows`, which computes the
//! visible row range from the scroll offset, so only on-screen tiles are laid
//! out and only those request their cover art.
//!
//! Album identity is the (name, artist) pair, since [`AlbumInfo`] carries no
//! stable id yet. A track belongs to an album when its album tag matches and
//! its artist tag matches (or is empty, which is how missing tags surface in
//! the mock data).

mod catalog;
#[cfg(test)]
mod tests;
mod thumbs;
mod tile;

use std::collections::HashMap;

use eframe::egui;

use self::catalog::{AlbumMeta, album_meta, album_tracks, sorted_albums};
use self::thumbs::ThumbnailCache;
use super::track_table::{self, TrackAction, TrackTableState};
use crate::library_api::{AlbumInfo, LibraryDataSource, TrackInfo};
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};

/// Tile edge-length bounds for the size slider, in pixels.
pub const MIN_TILE_SIZE: f32 = 96.0;
/// Maximum cover edge length selected with the size slider.
pub const MAX_TILE_SIZE: f32 = 256.0;
const DEFAULT_TILE_SIZE: f32 = 148.0;

/// Order the album grid is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlbumSort {
    #[default]
    Artist,
    Album,
    Year,
    RecentlyAdded,
}

impl AlbumSort {
    /// Every sort option, in menu order.
    pub const ALL: [Self; 4] = [Self::Artist, Self::Album, Self::Year, Self::RecentlyAdded];

    /// Label shown in the sort menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Year => "Year",
            Self::RecentlyAdded => "Recently added",
        }
    }
}

/// Identity of an album within the view; `AlbumInfo` has no id yet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlbumKey {
    name: String,
    artist: String,
}

impl AlbumKey {
    fn of(album: &AlbumInfo) -> Self {
        Self {
            name: album.name.clone(),
            artist: album.artist.clone(),
        }
    }

    /// Key for a track, or `None` when the artist tag is missing (the track
    /// cannot be attributed to a specific same-named album).
    fn of_track(track: &TrackInfo) -> Option<Self> {
        (!track.artist.is_empty()).then(|| Self {
            name: track.album.clone(),
            artist: track.artist.clone(),
        })
    }
}

/// Persistent album-grid state, stored on [`AppState`].
pub struct AlbumGridState {
    /// Cover edge length, driven by the size slider.
    pub tile_size: f32,
    /// Current sort order.
    pub sort: AlbumSort,
    /// Selected album, whose tracks are shown below the grid.
    pub selected: Option<AlbumKey>,
    /// The selected album's track table (sort + selection).
    pub table: TrackTableState,
    thumbs: ThumbnailCache,
}

impl Default for AlbumGridState {
    fn default() -> Self {
        Self {
            tile_size: DEFAULT_TILE_SIZE,
            sort: AlbumSort::default(),
            selected: None,
            table: TrackTableState::default(),
            thumbs: ThumbnailCache::default(),
        }
    }
}

/// Renders the album grid and, when an album is selected, its track table.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    if library.albums().is_empty() {
        empty_state(ui);
        return;
    }

    let meta = album_meta(library);
    let playing_id = currently_playing_id(library, player);
    let mut commands = Vec::new();

    let grid = &mut state.album_grid;
    let albums = sorted_albums(library, &meta, grid.sort);

    controls(ui, grid, albums.len());
    ui.separator();

    let selected_tracks = selected_album(grid, &albums).map(|album| album_tracks(library, album));
    match selected_tracks {
        Some(tracks) => {
            let grid_height = (ui.available_height() * 0.45).clamp(160.0, 360.0);
            ui.allocate_ui(egui::vec2(ui.available_width(), grid_height), |ui| {
                grid_view(ui, grid, &albums, &meta, library, &mut commands);
            });
            ui.separator();
            if let Some(action) =
                track_table::show(ui, "album_table", &mut grid.table, &tracks, playing_id)
            {
                commands.push(track_command(action));
            }
        }
        None => grid_view(ui, grid, &albums, &meta, library, &mut commands),
    }

    state.pending.append(&mut commands);
}

fn empty_state(ui: &mut egui::Ui) {
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        ui.heading("No albums yet");
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Scan a music folder to build your library.").weak());
    });
}

/// Sort menu, cover-size slider and the selected-album close button.
fn controls(ui: &mut egui::Ui, grid: &mut AlbumGridState, album_count: usize) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{album_count} albums")).weak());
        ui.separator();
        ui.label("Sort");
        egui::ComboBox::from_id_salt("album_sort")
            .selected_text(grid.sort.label())
            .show_ui(ui, |ui| {
                for sort in AlbumSort::ALL {
                    ui.selectable_value(&mut grid.sort, sort, sort.label());
                }
            });
        ui.separator();
        ui.label("Size");
        ui.add(
            egui::Slider::new(&mut grid.tile_size, MIN_TILE_SIZE..=MAX_TILE_SIZE).show_value(false),
        );
        if grid.selected.is_some() {
            ui.separator();
            if ui.button("Close album").clicked() {
                grid.selected = None;
            }
        }
    });
}

/// The virtualized grid itself. Only the visible rows are laid out; each tile
/// requests its cover from the thumbnail cache and, on a double-click, plays
/// the whole album.
fn grid_view(
    ui: &mut egui::Ui,
    grid: &mut AlbumGridState,
    albums: &[&AlbumInfo],
    meta: &HashMap<AlbumKey, AlbumMeta>,
    library: &dyn LibraryDataSource,
    commands: &mut Vec<Command>,
) {
    grid.thumbs.drain(ui.ctx());

    let spacing = ui.spacing().item_spacing.x;
    let tile = grid.tile_size;
    let columns = (((ui.available_width() + spacing) / (tile + spacing)).floor() as usize).max(1);
    let rows = albums.len().div_ceil(columns);

    egui::ScrollArea::vertical()
        .id_salt("album_grid_scroll")
        .show_rows(ui, tile + tile::CAPTION_HEIGHT, rows, |ui, row_range| {
            for row in row_range {
                ui.horizontal(|ui| {
                    for column in 0..columns {
                        let Some(album) = albums.get(row * columns + column) else {
                            break;
                        };
                        let key = AlbumKey::of(album);
                        let selected = grid.selected.as_ref() == Some(&key);
                        let response = {
                            let texture = meta
                                .get(&key)
                                .and_then(|meta| grid.thumbs.get(ui.ctx(), &meta.art_path));
                            tile::show(ui, album, texture, tile, selected)
                        };
                        if response.clicked() {
                            grid.selected = Some(key);
                        }
                        if response.double_clicked() {
                            let ids: Vec<u64> =
                                album_tracks(library, album).iter().map(|t| t.id).collect();
                            if !ids.is_empty() {
                                commands.push(Command::PlayAlbum(ids));
                            }
                        }
                        response.context_menu(|ui| {
                            if ui.button("Shuffle play").clicked() {
                                commands.push(crate::shuffle::album(library, album));
                                ui.close();
                            }
                        });
                    }
                });
            }
        });
}

fn track_command(action: TrackAction) -> Command {
    match action {
        TrackAction::Play { id, context } => Command::play_track(id, context),
        TrackAction::PlayNext(id) => Command::PlayTrackNext(id),
        TrackAction::AddToQueue(id) => Command::QueueTrack(id),
    }
}

/// The selected album's entry in the current sort order, if it still exists.
fn selected_album<'a>(grid: &AlbumGridState, albums: &[&'a AlbumInfo]) -> Option<&'a AlbumInfo> {
    let key = grid.selected.as_ref()?;
    albums
        .iter()
        .copied()
        .find(|album| AlbumKey::of(album) == *key)
}

fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library
        .track_by_path(&now_playing.path)
        .map(|track| track.id)
}
