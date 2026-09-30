//! Unit tests for the album-grid model's identity, ordering and revision
//! gating (#17, #332).

use super::*;
use crate::library_api::{AlbumInfo, TrackInfo};
use crate::mock::MockLibrary;
use crate::search::SearchEngine;
use crate::views::test_library::RiggedLibrary;

/// Ticks `engine` until it stops being pending, polling like the UI does.
fn settle(engine: &mut SearchEngine, tracks: &[TrackInfo], query: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        engine.tick(tracks, query);
        if !engine.is_pending() || std::time::Instant::now() > deadline {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// A library with two albums: "Alpha" by "A" (two tracks) and "Beta" by "B".
fn two_album_library() -> RiggedLibrary {
    let mut library = RiggedLibrary::new(1);
    library.albums = vec![
        AlbumInfo {
            name: "Alpha".to_string(),
            artist: "A".to_string(),
            year: None,
            track_count: 2,
        },
        AlbumInfo {
            name: "Beta".to_string(),
            artist: "B".to_string(),
            year: None,
            track_count: 1,
        },
    ];
    library.tracks = vec![
        TrackInfo {
            id: 1,
            title: "One".to_string(),
            album: "Alpha".to_string(),
            artist: "A".to_string(),
            ..TrackInfo::default()
        },
        TrackInfo {
            id: 2,
            title: "Two".to_string(),
            album: "Alpha".to_string(),
            artist: "A".to_string(),
            ..TrackInfo::default()
        },
        TrackInfo {
            id: 3,
            title: "Three".to_string(),
            album: "Beta".to_string(),
            artist: "B".to_string(),
            ..TrackInfo::default()
        },
    ];
    library
}

fn grid_with_search(library: &RiggedLibrary, engine: &SearchEngine) -> AlbumGrid {
    let mut grid = AlbumGrid::default();
    grid.refresh(&Ctx::with_library(&[], None, library).with_search(engine));
    grid
}

fn grid_with_library(library: &dyn crate::library_api::LibraryDataSource) -> AlbumGrid {
    let mut grid = AlbumGrid::default();
    grid.refresh(&Ctx::with_library(&[], None, library));
    grid
}

#[test]
fn refresh_populates_albums_and_sorts() {
    let library = MockLibrary::new();
    let source: &dyn crate::library_api::LibraryDataSource = &library;
    let grid = grid_with_library(source);
    assert_eq!(grid.len(), source.albums().len());
    // Default sort is by artist, case-insensitively.
    let artists: Vec<&str> = grid
        .albums()
        .iter()
        .map(|album| album.artist.as_str())
        .collect();
    let mut expected = artists.clone();
    expected.sort_by_key(|a| a.to_lowercase());
    assert_eq!(artists, expected);
}

#[test]
fn columns_for_matches_the_grid_layout_math() {
    let library = MockLibrary::new();
    let mut grid = grid_with_library(&library);
    grid.tile_size = 100.0;
    // (width + spacing) / (tile + spacing), floored.
    assert_eq!(grid.columns_for(430.0, 10.0), 4);
    assert_eq!(grid.columns_for(0.0, 10.0), 1, "always at least one column");
}

#[test]
fn tile_index_is_row_major_and_bounded() {
    let library = MockLibrary::new();
    let grid = grid_with_library(&library);
    let columns = 3;
    assert_eq!(grid.tile_index(0, 0, columns), Some(0));
    assert_eq!(grid.tile_index(0, 1, columns), Some(1));
    assert_eq!(grid.tile_index(1, 0, columns), Some(3));
    assert_eq!(grid.tile_index(usize::MAX, 0, columns), None);
}

#[test]
fn tile_clicked_selects_and_close_clears() {
    let library = MockLibrary::new();
    let mut grid = grid_with_library(&library);
    let key = AlbumKey::of(&grid.albums()[0]);
    let mut out = Commands::new();
    grid.update(
        AlbumGridMsg::TileClicked(key.clone()),
        &Ctx::with_library(&[], None, &library),
        &mut out,
    );
    assert_eq!(grid.selected_key(), Some(&key));
    assert!(grid.tile(0).is_some_and(|view| view.selected));

    grid.update(
        AlbumGridMsg::CloseAlbum,
        &Ctx::with_library(&[], None, &library),
        &mut out,
    );
    assert!(grid.selected_key().is_none());
}

#[test]
fn tile_activated_plays_the_albums_tracks() {
    let library = MockLibrary::new();
    let mut grid = grid_with_library(&library);
    let album = grid.albums()[0].clone();
    let expected: Vec<u64> = album_tracks(&library, &album)
        .into_iter()
        .map(|track| track.id)
        .collect();

    let mut out = Commands::new();
    grid.update(
        AlbumGridMsg::TileActivated(AlbumKey::of(&album)),
        &Ctx::with_library(&[], None, &library),
        &mut out,
    );
    assert_eq!(out.into_vec(), vec![Command::PlayAlbum(expected)]);
}

#[test]
fn refresh_with_unchanged_library_is_a_no_op() {
    let mut library = RiggedLibrary::new(7);
    library.albums = vec![AlbumInfo {
        name: "Record".to_string(),
        artist: "Artist".to_string(),
        year: Some(2020),
        track_count: 1,
    }];
    library.tracks = vec![TrackInfo {
        id: 1,
        album: "Record".to_string(),
        artist: "Artist".to_string(),
        ..TrackInfo::default()
    }];
    let cx = Ctx::with_library(&[], None, &library);
    let mut grid = AlbumGrid::default();
    grid.refresh(&cx);
    assert_eq!(grid.len(), 1, "the first refresh builds the list");
    let album_reads = library.album_reads.get();
    let track_reads = library.track_reads.get();

    grid.refresh(&cx);

    assert_eq!(
        library.album_reads.get(),
        album_reads,
        "an unchanged library must not clone the album list"
    );
    assert_eq!(
        library.track_reads.get(),
        track_reads,
        "an unchanged library must not rebuild album_meta"
    );
}

#[test]
fn sort_change_reorders_and_bumps_the_list_revision() {
    let library = MockLibrary::new();
    let cx = Ctx::with_library(&[], None, &library);
    let mut grid = grid_with_library(&library);
    let before = grid.list_revision();

    grid.update(
        AlbumGridMsg::SetSort(AlbumSort::Year),
        &cx,
        &mut Commands::new(),
    );
    grid.refresh(&cx);

    assert!(
        grid.list_revision() > before,
        "a sort change rebuilds the list"
    );
}

#[test]
fn selecting_a_tile_only_bumps_the_selection_revision() {
    let library = MockLibrary::new();
    let cx = Ctx::with_library(&[], None, &library);
    let mut grid = grid_with_library(&library);
    let list = grid.list_revision();

    let key = AlbumKey::of(&grid.albums()[0]);
    grid.update(AlbumGridMsg::TileClicked(key), &cx, &mut Commands::new());
    grid.refresh(&cx);

    assert_eq!(
        grid.list_revision(),
        list,
        "selecting a tile must not rebuild the album list"
    );
    assert!(
        grid.selection_revision() > 0,
        "selecting a tile rebuilds the selected tracks"
    );
}

#[test]
fn tile_size_change_does_not_rebuild_the_list() {
    let library = MockLibrary::new();
    let cx = Ctx::with_library(&[], None, &library);
    let mut grid = grid_with_library(&library);
    let list = grid.list_revision();

    grid.update(
        AlbumGridMsg::SetTileSize(grid.tile_size + 10.0),
        &cx,
        &mut Commands::new(),
    );
    grid.refresh(&cx);

    assert_eq!(grid.list_revision(), list);
}

#[test]
fn search_keeps_only_albums_with_a_matching_track_and_restores_on_clear() {
    let library = two_album_library();
    let mut engine = SearchEngine::new();
    settle(&mut engine, &library.tracks, "one");

    let grid = grid_with_search(&library, &engine);
    assert_eq!(grid.len(), 1, "only the album with the matching track");
    assert_eq!(grid.albums()[0].name, "Alpha");

    settle(&mut engine, &library.tracks, "");
    let restored = grid_with_search(&library, &engine);
    assert_eq!(restored.len(), 2, "clearing the query restores every album");
}

#[test]
fn an_album_with_one_matching_track_among_many_is_kept_once() {
    let library = two_album_library();
    let mut engine = SearchEngine::new();
    settle(&mut engine, &library.tracks, "one");

    let grid = grid_with_search(&library, &engine);
    let alpha = grid
        .albums()
        .iter()
        .filter(|album| album.name == "Alpha")
        .count();
    assert_eq!(
        alpha, 1,
        "an album appears once regardless of its track hits"
    );
}

#[test]
fn search_keeps_an_album_whose_name_matches_without_a_track_hit() {
    let library = two_album_library();
    let mut engine = SearchEngine::new();
    settle(&mut engine, &library.tracks, "beta");

    let grid = grid_with_search(&library, &engine);
    assert_eq!(grid.len(), 1);
    assert_eq!(grid.albums()[0].name, "Beta");
}

#[test]
fn a_query_matching_nothing_yields_no_albums() {
    let library = two_album_library();
    let mut engine = SearchEngine::new();
    settle(&mut engine, &library.tracks, "zzzz");

    let grid = grid_with_search(&library, &engine);
    assert!(grid.is_empty());
}

#[test]
fn a_search_change_rebuilds_the_list_without_a_library_change() {
    let library = two_album_library();
    let mut grid = AlbumGrid::default();
    grid.refresh(&Ctx::with_library(&[], None, &library));
    let revision = grid.list_revision();

    let mut engine = SearchEngine::new();
    settle(&mut engine, &library.tracks, "one");
    grid.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
    assert!(
        grid.list_revision() > revision,
        "a search change rebuilds the album list even when the library did not"
    );
}
