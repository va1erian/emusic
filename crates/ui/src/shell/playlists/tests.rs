use std::time::Instant;

use emusic_player::QueueSnapshot;

use crate::config::Config;
use crate::mock::{MockLibrary, MockPlayer};
use crate::shell::Shell;
use crate::state::{Command, View};
use crate::waker::WakerSlot;

fn shell() -> Shell {
    Shell::new(
        Box::new(MockLibrary::new()),
        Box::new(MockPlayer::default()),
        Config::default(),
        None,
        WakerSlot::new(),
    )
}

fn run(shell: &mut Shell, commands: impl IntoIterator<Item = Command>) {
    for command in commands {
        shell.dispatch(command);
    }
    shell.tick(Instant::now());
}

/// Creates a playlist holding the library's first three tracks.
fn with_playlist(shell: &mut Shell) -> (u64, Vec<u64>) {
    run(shell, [Command::CreatePlaylist("Mix".into())]);
    let id = shell.state.selected_playlist.unwrap();
    let tracks: Vec<u64> = shell
        .library
        .tracks()
        .iter()
        .take(3)
        .map(|t| t.id)
        .collect();
    run(
        shell,
        [Command::AddToPlaylist {
            id,
            tracks: tracks.clone(),
        }],
    );
    (id, tracks)
}

#[test]
fn creating_a_playlist_selects_it_and_names_blank_ones() {
    let mut shell = shell();
    run(&mut shell, [Command::CreatePlaylist("  ".into())]);

    assert_eq!(shell.state.view, View::Playlist);
    let lists = shell.library.playlists();
    assert_eq!(lists[0].name, "New playlist");
    assert_eq!(shell.state.selected_playlist, Some(lists[0].id));
    assert_eq!(shell.state.navigator.playlists().len(), 1);
    // Frontends sync the shown view into the navigator each frame.
    shell.state.navigator.sync(shell.state.view);
    assert_eq!(
        shell.state.navigator.highlighted_playlist(),
        shell.state.selected_playlist
    );
}

#[test]
fn rename_ignores_blank_names_and_trims() {
    let mut shell = shell();
    run(&mut shell, [Command::CreatePlaylist("Old".into())]);
    let id = shell.state.selected_playlist.unwrap();

    run(
        &mut shell,
        [Command::RenamePlaylist {
            id,
            name: " ".into(),
        }],
    );
    assert_eq!(shell.library.playlists()[0].name, "Old");
    run(
        &mut shell,
        [Command::RenamePlaylist {
            id,
            name: " New ".into(),
        }],
    );
    assert_eq!(shell.library.playlists()[0].name, "New");
}

#[test]
fn add_remove_and_move_edit_the_playlist() {
    let mut shell = shell();
    let (id, tracks) = with_playlist(&mut shell);
    let order = |shell: &Shell| -> Vec<u64> {
        shell
            .library
            .playlist_items(id)
            .iter()
            .map(|i| i.track_id)
            .collect()
    };
    assert_eq!(order(&shell), tracks);

    let entries: Vec<i64> = shell
        .library
        .playlist_items(id)
        .iter()
        .map(|i| i.entry_id)
        .collect();
    run(
        &mut shell,
        [Command::MoveInPlaylist {
            id,
            entries: vec![entries[2]],
            to: 0,
        }],
    );
    assert_eq!(order(&shell), [tracks[2], tracks[0], tracks[1]]);

    run(
        &mut shell,
        [Command::RemoveFromPlaylist {
            id,
            entries: vec![entries[0]],
        }],
    );
    assert_eq!(order(&shell), [tracks[2], tracks[1]]);
}

#[test]
fn deleting_the_selected_playlist_returns_to_music() {
    let mut shell = shell();
    let (id, _) = with_playlist(&mut shell);
    run(&mut shell, [Command::DeletePlaylist(id)]);

    assert!(shell.library.playlists().is_empty());
    assert_eq!(shell.state.selected_playlist, None);
    assert_eq!(shell.state.view, View::Music);
}

#[test]
fn a_stale_selected_playlist_falls_back_to_music() {
    let mut shell = shell();
    run(&mut shell, [Command::SelectPlaylist(404)]);
    assert_eq!(shell.state.view, View::Music);
    assert_eq!(shell.state.selected_playlist, None);
}

#[test]
fn selecting_an_existing_playlist_shows_it() {
    let mut shell = shell();
    let (id, _) = with_playlist(&mut shell);
    run(&mut shell, [Command::SetView(View::Albums)]);
    run(&mut shell, [Command::SelectPlaylist(id)]);
    assert_eq!(shell.state.view, View::Playlist);
    assert_eq!(shell.state.selected_playlist, Some(id));
}

#[test]
fn play_playlist_queues_its_tracks_in_order() {
    let mut shell = shell();
    let (id, tracks) = with_playlist(&mut shell);
    run(&mut shell, [Command::PlayPlaylist { id, shuffle: false }]);

    let path = |track: u64| {
        shell
            .library
            .tracks()
            .iter()
            .find(|t| t.id == track)
            .unwrap()
            .path
            .clone()
    };
    assert_eq!(shell.player.now_playing().unwrap().path, path(tracks[0]));
    assert_eq!(
        shell.player.queue().len(),
        2,
        "the remaining tracks are queued"
    );
}

#[test]
fn play_playlist_shuffled_starts_a_shuffle_scope_named_after_it() {
    let mut shell = shell();
    let (id, _) = with_playlist(&mut shell);
    run(&mut shell, [Command::PlayPlaylist { id, shuffle: true }]);

    assert!(shell.player.shuffle());
    let QueueSnapshot::Shuffle(snapshot) = shell.player.queue_snapshot() else {
        panic!("expected a shuffle queue");
    };
    assert_eq!(snapshot.label, "Mix");
}

#[test]
fn playing_an_empty_or_missing_playlist_does_nothing() {
    let mut shell = shell();
    run(&mut shell, [Command::CreatePlaylist("Empty".into())]);
    let id = shell.state.selected_playlist.unwrap();
    run(&mut shell, [Command::PlayPlaylist { id, shuffle: false }]);
    run(
        &mut shell,
        [Command::PlayPlaylist {
            id: 999,
            shuffle: true,
        }],
    );
    assert!(shell.player.now_playing().is_none());
}

#[test]
fn export_writes_an_m3u_file_next_to_the_music() {
    let mut shell = shell();
    let (id, tracks) = with_playlist(&mut shell);
    let dir = std::env::temp_dir().join(format!("emusic-m3u-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("mix.m3u8");

    run(
        &mut shell,
        [Command::ExportPlaylist {
            id,
            path: file.clone(),
        }],
    );

    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(text.starts_with("#EXTM3U\n"));
    assert_eq!(text.matches("#EXTINF:").count(), tracks.len());
}
