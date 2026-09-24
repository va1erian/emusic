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
//!
//! The view state is toolkit-agnostic (`emusic_ui`, #97); the thumbnail cache
//! is egui-bound (it owns GPU handles via [`EguiImageSink`], #96) and is
//! passed in by the frontend.

#[cfg(test)]
mod tests;
pub(crate) mod thumbs;
mod tile;

use std::collections::HashMap;

use eframe::egui;

use self::thumbs::ThumbnailCache;
use super::EguiView;
use crate::image_sink::EguiImageSink;
use crate::library_api::{AlbumInfo, LibraryDataSource};
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};
pub use emusic_ui::views::album_grid::AlbumGridState;
use emusic_ui::views::album_grid::catalog::{AlbumMeta, album_meta, album_tracks, sorted_albums};
use emusic_ui::views::album_grid::models::{AlbumKey, AlbumSort};
use emusic_ui::views::{Commands, Ctx};

/// Tile edge-length bounds for the size slider, in pixels.
pub const MIN_TILE_SIZE: f32 = 96.0;
/// Maximum cover edge length selected with the size slider.
pub const MAX_TILE_SIZE: f32 = 256.0;
/// Smallest height the cover grid is given, whatever space is left.
const MIN_GRID_HEIGHT: f32 = 160.0;

/// Renders the album grid and, when an album is selected, its track table.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    thumbs: &mut ThumbnailCache,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    if library.albums().is_empty() {
        empty_state(ui);
        return;
    }

    let meta = album_meta(library);
    let playing_id = currently_playing_id(library, player);
    let mut commands = Commands::new();

    let grid = &mut state.album_grid;
    let albums = sorted_albums(library, &meta, grid.sort);

    controls(ui, grid, albums.len());
    ui.separator();

    let selected_tracks = selected_album(grid, &albums).map(|album| album_tracks(library, album));
    // Give the grid a definite height so its virtualization only lays out (and
    // loads covers for) the visible tiles.
    let visible_height = ui.available_height().max(MIN_GRID_HEIGHT);
    match selected_tracks {
        Some(tracks) => {
            let grid_height = (visible_height * 0.45).clamp(MIN_GRID_HEIGHT, 360.0);
            ui.allocate_ui(egui::vec2(ui.available_width(), grid_height), |ui| {
                grid_view(ui, grid, thumbs, &albums, &meta, library, &mut commands);
            });
            ui.separator();
            let cx = Ctx::new(&tracks, playing_id);
            grid.table.show(ui, "album_table", &cx, &mut commands);
        }
        None => {
            ui.allocate_ui(egui::vec2(ui.available_width(), visible_height), |ui| {
                grid_view(ui, grid, thumbs, &albums, &meta, library, &mut commands);
            });
        }
    }

    state.pending.extend(commands.into_vec());
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
    thumbs: &mut ThumbnailCache,
    albums: &[&AlbumInfo],
    meta: &HashMap<AlbumKey, AlbumMeta>,
    library: &dyn LibraryDataSource,
    commands: &mut Commands,
) {
    let mut sink = EguiImageSink::new(ui.ctx().clone(), "album_thumb");
    thumbs.drain(&mut sink);

    let spacing = ui.spacing().item_spacing.x;
    let tile = grid.tile_size;
    // Reserve the vertical scroll bar's width, or the last column of each row
    // overflows the scroll area and triggers a horizontal scroll bar.
    let width = ui.available_width() - ui.spacing().scroll.allocated_width();
    let columns = (((width + spacing) / (tile + spacing)).floor() as usize).max(1);
    let rows = albums.len().div_ceil(columns);

    egui::ScrollArea::vertical()
        .id_salt("album_grid_scroll")
        .auto_shrink([false, false])
        .show_rows(ui, tile + tile::CAPTION_HEIGHT, rows, |ui, row_range| {
            for row in row_range {
                ui.horizontal(|ui| {
                    for column in 0..columns {
                        let Some(album) = albums.get(row * columns + column) else {
                            break;
                        };
                        let key = AlbumKey::of(album);
                        let selected = grid.selected.as_ref() == Some(&key);
                        let texture = meta
                            .get(&key)
                            .and_then(|meta| thumbs.get(&mut sink, &meta.art_path));
                        let response = tile::show(ui, album, texture, tile, selected);
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
