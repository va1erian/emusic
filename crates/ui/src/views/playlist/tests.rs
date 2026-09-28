use super::*;
use crate::mock::MockLibrary;
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
    view.refresh(&resolved.items, &Ctx::new(&resolved.tracks, None));
    resolved.tracks.iter().map(|t| t.id).collect()
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
    view.move_rows(id, &[3, 99], 0, &mut out);
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
    view.move_rows(id, &[3], 0, &mut out);
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
