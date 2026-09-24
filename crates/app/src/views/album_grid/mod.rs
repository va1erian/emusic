//! egui renderer for the "Albums" view (#17, #100): a virtualized grid of
//! cover tiles, backed by an off-thread thumbnail cache, with the selected
//! album's tracks shown in the shared track table below.
//!
//! All state and logic live in [`AlbumGrid`] (`emusic-ui`); this module only
//! draws. The grid is virtualized by `ScrollArea::show_rows` over the rows
//! the model computes (`columns_for`/`rows`), so only on-screen tiles are laid
//! out and only those request their cover art.
//!
//! The thumbnail cache is egui-bound (it owns GPU handles via
//! [`EguiImageSink`], #96) and is passed in by the frontend.

#[cfg(test)]
mod tests;
pub(crate) mod thumbs;
mod tile;

use eframe::egui;

use self::thumbs::ThumbnailCache;
use super::EguiView;
use crate::image_sink::EguiImageSink;
use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::PlayerApi;
use crate::state::AppState;
use emusic_ui::views::album_grid::models::{AlbumKey, AlbumSort};
use emusic_ui::views::album_grid::{AlbumGrid, AlbumGridMsg};
use emusic_ui::views::{Commands, Ctx};

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

    let playing_id = currently_playing_id(library, player);
    let mut commands = Commands::new();

    let grid = &mut state.album_grid;
    let mut control_msgs = Vec::new();
    grid.refresh(&Ctx::with_library(&[], playing_id, library));

    let album_count = grid.len();
    controls(ui, grid, album_count, &mut control_msgs);
    let control_cx = Ctx::with_library(&[], playing_id, library);
    for msg in control_msgs {
        grid.update(msg, &control_cx, &mut commands);
    }
    ui.separator();

    let selected_ids = grid.selected_track_ids().to_vec();
    let selected_tracks: Vec<&TrackInfo> = selected_ids
        .iter()
        .filter_map(|id| library.tracks().iter().find(|track| track.id == *id))
        .collect();

    // Give the grid a definite height so its virtualization only lays out (and
    // loads covers for) the visible tiles.
    let visible_height = ui.available_height().max(MIN_GRID_HEIGHT);
    if selected_tracks.is_empty() {
        ui.allocate_ui(egui::vec2(ui.available_width(), visible_height), |ui| {
            grid_view(ui, grid, thumbs, library, playing_id, &mut commands);
        });
    } else {
        let grid_height = (visible_height * 0.45).clamp(MIN_GRID_HEIGHT, 360.0);
        ui.allocate_ui(egui::vec2(ui.available_width(), grid_height), |ui| {
            grid_view(ui, grid, thumbs, library, playing_id, &mut commands);
        });
        ui.separator();
        let cx = Ctx::new(&selected_tracks, playing_id);
        grid.table.show(ui, "album_table", &cx, &mut commands);
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
/// Records intents as messages so the model stays the source of truth.
fn controls(
    ui: &mut egui::Ui,
    grid: &AlbumGrid,
    album_count: usize,
    messages: &mut Vec<AlbumGridMsg>,
) {
    use emusic_ui::views::album_grid::{MAX_TILE_SIZE, MIN_TILE_SIZE};

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{album_count} albums")).weak());
        ui.separator();
        ui.label("Sort");
        egui::ComboBox::from_id_salt("album_sort")
            .selected_text(grid.sort.label())
            .show_ui(ui, |ui| {
                for sort in AlbumSort::ALL {
                    if ui
                        .selectable_label(grid.sort == sort, sort.label())
                        .clicked()
                    {
                        messages.push(AlbumGridMsg::SetSort(sort));
                    }
                }
            });
        ui.separator();
        ui.label("Size");
        let mut tile_size = grid.tile_size;
        if ui
            .add(egui::Slider::new(&mut tile_size, MIN_TILE_SIZE..=MAX_TILE_SIZE).show_value(false))
            .changed()
        {
            messages.push(AlbumGridMsg::SetTileSize(tile_size));
        }
        if grid.selected.is_some() {
            ui.separator();
            if ui.button("Close album").clicked() {
                messages.push(AlbumGridMsg::CloseAlbum);
            }
        }
    });
}

/// The virtualized grid itself. Only the visible rows are laid out; each tile
/// requests its cover from the thumbnail cache and, on a double-click, plays
/// the whole album.
fn grid_view(
    ui: &mut egui::Ui,
    grid: &mut AlbumGrid,
    thumbs: &mut ThumbnailCache,
    library: &dyn LibraryDataSource,
    playing_id: Option<u64>,
    commands: &mut Commands,
) {
    let mut sink = EguiImageSink::new(ui.ctx().clone(), "album_thumb");
    thumbs.drain(&mut sink);

    let meta = emusic_ui::views::album_grid::album_meta(library);
    let spacing = ui.spacing().item_spacing.x;
    let tile = grid.tile_size;
    // Reserve the vertical scroll bar's width, or the last column of each row
    // overflows the scroll area and triggers a horizontal scroll bar.
    let width = ui.available_width() - ui.spacing().scroll.allocated_width();
    let columns = grid.columns_for(width, spacing);
    let rows = grid.rows(columns);

    egui::ScrollArea::vertical()
        .id_salt("album_grid_scroll")
        .auto_shrink([false, false])
        .show_rows(ui, tile + tile::CAPTION_HEIGHT, rows, |ui, row_range| {
            for row in row_range {
                ui.horizontal(|ui| {
                    for column in 0..columns {
                        let Some(index) = grid.tile_index(row, column, columns) else {
                            break;
                        };
                        let Some(view) = grid.tile(index) else {
                            break;
                        };
                        let key = AlbumKey::of(view.album);
                        let texture = meta
                            .get(&key)
                            .and_then(|meta| thumbs.get(&mut sink, &meta.art_path));
                        let response = tile::show(ui, view.album, texture, tile, view.selected);
                        if response.clicked() {
                            grid.selected = Some(key.clone());
                        }
                        if response.double_clicked() {
                            let cx = Ctx::with_library(&[], playing_id, library);
                            grid.update(
                                emusic_ui::views::album_grid::AlbumGridMsg::TileActivated(
                                    key.clone(),
                                ),
                                &cx,
                                commands,
                            );
                        }
                        response.context_menu(|ui| {
                            if ui.button("Shuffle play").clicked() {
                                let cx = Ctx::with_library(&[], playing_id, library);
                                grid.update(
                                    emusic_ui::views::album_grid::AlbumGridMsg::Shuffle(
                                        key.clone(),
                                    ),
                                    &cx,
                                    commands,
                                );
                                ui.close();
                            }
                        });
                    }
                });
            }
        });
}

fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library
        .track_by_path(&now_playing.path)
        .map(|track| track.id)
}
