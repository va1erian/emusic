use super::*;
use crate::mock::MockLibrary;
use crate::search::SearchEngine;
use crate::views::track_table::selection::ClickModifiers;

/// A mock library with a playlist holding its first three tracks plus a
/// repeat of the first, and a stale entry for a track that does not exist.
fn library_with_playlist() -> (MockLibrary, u64, [u64; 3]) {
    let mut library = MockLibrary::new();
    let ids: Vec<u64> = library.tracks().iter().take(3).map(|t| t.id).collect();
    let playlist = library.create_playlist("Mix").unwrap();
    library.add_to_playlist(playlist, &[ids[0], ids[1], u64::MAX, ids[2], ids[0]]);
    (library, playlist, [ids[0], ids[1], ids[2]])
}

fn refreshed(library: &MockLibrary, id: u64, view: &mut PlaylistView) -> Vec<u64> {
    let resolved = resolve(library, id);
    let visible = view.refresh(&resolved.items, &resolved.tracks, None, None);
    visible.iter().map(|t| t.id).collect()
}

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

/// The file name of track `id`, unique enough to isolate it in a search.
fn filename_of(library: &MockLibrary, id: u64) -> String {
    library
        .tracks()
        .iter()
        .find(|track| track.id == id)
        .unwrap()
        .path
        .rsplit('/')
        .next()
        .unwrap()
        .to_string()
}

#[test]
fn resolve_keeps_playlist_order_and_drops_missing_tracks() {
    let (library, id, [a, b, c]) = library_with_playlist();
    let resolved = resolve(&library, id);
    let tracks: Vec<u64> = resolved.tracks.iter().map(|t| t.id).collect();
    assert_eq!(tracks, [a, b, c, a]);
    assert_eq!(resolved.items.len(), 4, "items stay aligned with tracks");
    assert!(resolve(&library, 999).items.is_empty());
}

#[test]
fn refresh_bumps_the_revision_only_when_entries_change() {
    let (mut library, id, _) = library_with_playlist();
    let mut view = PlaylistView::default();
    refreshed(&library, id, &mut view);
    let first = view.revision();
    assert_eq!(view.count(), 4);
    assert_eq!(view.count_label(), "4 tracks");

    refreshed(&library, id, &mut view);
    assert_eq!(view.revision(), first, "a stable playlist doesn't bump");

    let entry = view.items()[0].entry_id;
    library.remove_from_playlist(id, &[entry]);
    refreshed(&library, id, &mut view);
    assert!(view.revision() > first);
    assert_eq!(view.count(), 3);
}

#[test]
fn count_label_is_singular_for_one_track() {
    let (mut library, _, [a, ..]) = library_with_playlist();
    let id = library.create_playlist("One").unwrap();
    library.add_to_playlist(id, &[a]);
    let mut view = PlaylistView::default();
    refreshed(&library, id, &mut view);
    assert_eq!(view.count_label(), "1 track");
}

#[test]
fn remove_selected_targets_every_entry_of_the_selected_tracks() {
    let (library, id, [a, ..]) = library_with_playlist();
    let mut view = PlaylistView::default();
    let ids = refreshed(&library, id, &mut view);
    view.table
        .selection
        .click(&ids, 0, a, ClickModifiers::default());

    let mut out = Commands::new();
    view.remove_selected(id, &mut out);
    let first = view.items()[0].entry_id;
    let last = view.items()[3].entry_id;
    assert_eq!(
        out.into_vec(),
        vec![Command::RemoveFromPlaylist {
            id,
            entries: vec![first, last]
        }]
    );
}

#[test]
fn remove_selected_without_a_selection_does_nothing() {
    let (library, id, _) = library_with_playlist();
    let mut view = PlaylistView::default();
    refreshed(&library, id, &mut view);
    let mut out = Commands::new();
    view.remove_selected(id, &mut out);
    assert!(out.into_vec().is_empty());
}

#[test]
fn move_rows_maps_rows_to_entries_and_needs_playlist_order() {
    let (library, id, _) = library_with_playlist();
    let mut view = PlaylistView::default();
    refreshed(&library, id, &mut view);
    let entries: Vec<i64> = view.items().iter().map(|i| i.entry_id).collect();

    let mut out = Commands::new();
    view.move_rows(id, &[3, 99], 0, library.playlist_items(id), &mut out);
    assert_eq!(
        out.into_vec(),
        vec![Command::MoveInPlaylist {
            id,
            entries: vec![entries[3]],
            to: 0
        }]
    );

    view.table
        .sort
        .toggle(crate::views::track_table::columns::ColumnId::Title);
    assert!(!view.can_reorder());
    let mut out = Commands::new();
    view.move_rows(id, &[3], 0, library.playlist_items(id), &mut out);
    assert!(out.into_vec().is_empty(), "no reordering while sorted");
}

#[test]
fn play_emits_the_playlist_command() {
    let view = PlaylistView::default();
    let mut out = Commands::new();
    view.play(4, true, &mut out);
    assert_eq!(
        out.into_vec(),
        vec![Command::PlayPlaylist {
            id: 4,
            shuffle: true
        }]
    );
}

#[test]
fn move_rows_translates_the_target_past_hidden_entries() {
    // The stale entry (unknown track) is the third of five, so display index 3
    // (before the fourth visible row) is index 4 of the full list.
    let (library, id, _) = library_with_playlist();
    let mut view = PlaylistView::default();
    refreshed(&library, id, &mut view);
    let all = library.playlist_items(id);
    assert_eq!((all.len(), view.count()), (5, 4));

    let mut out = Commands::new();
    view.move_rows(id, &[0], 3, all, &mut out);
    let mut end = Commands::new();
    view.move_rows(id, &[0], 4, all, &mut end);

    let to = |commands: Vec<Command>| match &commands[0] {
        Command::MoveInPlaylist { to, .. } => *to,
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(to(out.into_vec()), 4);
    assert_eq!(to(end.into_vec()), 5, "past the last row appends");
}

#[test]
fn search_keeps_only_matching_entries_and_restores_on_clear() {
    let (library, id, [_a, b, _c]) = library_with_playlist();
    let needle = filename_of(&library, b);
    let mut engine = SearchEngine::new();
    settle(&mut engine, library.tracks(), &needle);

    let resolved = resolve(&library, id);
    let mut view = PlaylistView::default();
    let visible = view.refresh(&resolved.items, &resolved.tracks, Some(&engine), None);
    assert_eq!(visible.iter().map(|t| t.id).collect::<Vec<_>>(), vec![b]);
    assert_eq!(view.count(), 1);
    assert!(!view.can_reorder(), "reordering is disabled while filtered");

    settle(&mut engine, library.tracks(), "");
    let visible = view.refresh(&resolved.items, &resolved.tracks, Some(&engine), None);
    assert_eq!(visible.len(), 4, "clearing the query restores every entry");
    assert!(view.can_reorder());
}

#[test]
fn remove_selected_under_a_filter_ignores_hidden_entries() {
    let (library, id, [a, b, _c]) = library_with_playlist();
    let needle = filename_of(&library, b);
    let mut engine = SearchEngine::new();
    settle(&mut engine, library.tracks(), &needle);

    let resolved = resolve(&library, id);
    let order: Vec<u64> = resolved.tracks.iter().map(|t| t.id).collect();
    let mut view = PlaylistView::default();
    view.refresh(&resolved.items, &resolved.tracks, Some(&engine), None);

    // A selection on the hidden track removes nothing.
    view.table
        .selection
        .click(&order, 0, a, ClickModifiers::default());
    let mut out = Commands::new();
    view.remove_selected(id, &mut out);
    assert!(out.into_vec().is_empty());

    // A selection on the visible track removes exactly its entry.
    view.table
        .selection
        .click(&order, 0, b, ClickModifiers::default());
    let mut out = Commands::new();
    view.remove_selected(id, &mut out);
    let b_entry = resolved
        .items
        .iter()
        .find(|item| item.track_id == b)
        .map(|item| item.entry_id)
        .unwrap();
    assert_eq!(
        out.into_vec(),
        vec![Command::RemoveFromPlaylist {
            id,
            entries: vec![b_entry]
        }]
    );
}

#[test]
fn reordering_is_disabled_while_filtered() {
    let (library, id, [_a, b, _c]) = library_with_playlist();
    let needle = filename_of(&library, b);
    let mut engine = SearchEngine::new();
    settle(&mut engine, library.tracks(), &needle);

    let resolved = resolve(&library, id);
    let mut view = PlaylistView::default();
    view.refresh(&resolved.items, &resolved.tracks, Some(&engine), None);

    let mut out = Commands::new();
    view.move_rows(id, &[0], 0, library.playlist_items(id), &mut out);
    assert!(
        out.into_vec().is_empty(),
        "no reorder while a filter hides entries"
    );
}
