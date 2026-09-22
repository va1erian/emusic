//! Unit tests for the mock library's directory-tree builder.

use crate::library_api::TrackInfo;

use super::build;

fn track(path: &str) -> TrackInfo {
    TrackInfo {
        path: path.to_string(),
        ..TrackInfo::default()
    }
}

#[test]
fn groups_tracks_into_a_nested_tree() {
    let tracks = [
        track("D:/Music/Artist/Album/1.flac"),
        track("D:/Music/Artist/Album/2.flac"),
        track("D:/Music/Artist/3.flac"),
        track("D:/Music/Other/4.flac"),
    ];

    let roots = build(&tracks);
    assert_eq!(roots.len(), 1);
    let music = &roots[0].children[0];
    assert_eq!(music.name, "Music");
    assert_eq!(music.total_track_count, 4);
    assert_eq!(music.children.len(), 2);

    let artist = &music.children[0];
    assert_eq!(artist.name, "Artist");
    assert_eq!(artist.direct_track_count, 1);
    assert_eq!(artist.total_track_count, 3);
    assert_eq!(artist.children[0].name, "Album");
    assert_eq!(artist.children[0].direct_track_count, 2);
}

#[test]
fn children_are_sorted_case_insensitively() {
    let tracks = [
        track("D:/music/banana/a.flac"),
        track("D:/music/Apple/a.flac"),
    ];
    let roots = build(&tracks);
    let names: Vec<_> = roots[0].children[0]
        .children
        .iter()
        .map(|node| node.name.as_str())
        .collect();
    assert_eq!(names, ["Apple", "banana"]);
}
