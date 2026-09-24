//! Unit tests for the tag editor's form conversion and result handling.

use std::path::PathBuf;

use emusic_library::LibraryError;

use super::form::TagForm;
use super::{Status, TagEditorState, deliver};
use crate::library_api::{EditOutcome, TrackInfo};

fn track() -> TrackInfo {
    TrackInfo {
        id: 1,
        title: "Old title".to_string(),
        artist: "Old artist".to_string(),
        album: "Old album".to_string(),
        album_artist: "Old album artist".to_string(),
        genre: "Old genre".to_string(),
        year: Some(1999),
        track_no: Some(3),
        disc_no: Some(1),
        composer: "Old composer".to_string(),
        comment: "Old comment".to_string(),
        path: "C:/music/track.flac".to_string(),
        ..TrackInfo::default()
    }
}

#[test]
fn from_track_renders_optional_numbers_as_text() {
    let form = TagForm::from_track(&track());
    assert_eq!(form.year, "1999");
    assert_eq!(form.track_no, "3");
    assert_eq!(form.disc_no, "1");
    assert_eq!(form.title, "Old title");

    let mut empty = track();
    empty.year = None;
    empty.track_no = None;
    empty.disc_no = None;
    let form = TagForm::from_track(&empty);
    assert_eq!(form.year, "");
    assert_eq!(form.track_no, "");
    assert_eq!(form.disc_no, "");
}

#[test]
fn blank_text_fields_clear_the_tag() {
    let mut form = TagForm::from_track(&track());
    form.title = "   ".to_string();
    form.artist = "  Kept  ".to_string();

    let tags = form.to_tags().expect("valid form");
    assert_eq!(tags.title, None, "blank text clears the tag");
    assert_eq!(tags.artist.as_deref(), Some("Kept"), "values are trimmed");
}

#[test]
fn numbers_parse_and_blank_clears() {
    let mut form = TagForm::from_track(&track());
    form.year = "2024".to_string();
    form.track_no = "".to_string();
    form.disc_no = " 2 ".to_string();

    let tags = form.to_tags().expect("valid form");
    assert_eq!(tags.year, Some(2024));
    assert_eq!(tags.track_no, None);
    assert_eq!(tags.disc_no, Some(2));
}

#[test]
fn non_numeric_fields_report_errors() {
    let mut form = TagForm::from_track(&track());
    form.year = "nineteen".to_string();
    form.track_no = "1.5".to_string();
    form.disc_no = "-1".to_string();

    let errors = form.validate();
    assert!(errors.year.is_some());
    assert!(errors.track_no.is_some());
    assert!(errors.disc_no.is_some(), "disc is unsigned");
    assert!(form.to_tags().is_err());
}

#[test]
fn deliver_success_marks_saved_and_updates_original() {
    let track = track();
    let mut state = Some(TagEditorState::new(&track));
    state.as_mut().expect("editor is open").form.title = "New title".to_string();

    deliver(
        &mut state,
        vec![EditOutcome {
            path: PathBuf::from(&track.path),
            result: Ok(()),
        }],
    );

    let editor = state.as_ref().expect("editor stays open");
    assert!(matches!(editor.status, Status::Saved));
    assert_eq!(editor.original.title, "New title");
}

#[test]
fn deliver_failure_lists_the_file_and_keeps_original() {
    let track = track();
    let mut state = Some(TagEditorState::new(&track));
    state.as_mut().expect("editor is open").form.title = "New title".to_string();

    deliver(
        &mut state,
        vec![EditOutcome {
            path: PathBuf::from(&track.path),
            result: Err(LibraryError::NoDataDir),
        }],
    );

    let editor = state.as_ref().expect("editor stays open");
    let Status::Failed(failures) = &editor.status else {
        panic!("expected a failure status");
    };
    assert_eq!(failures.len(), 1);
    assert!(failures[0].message.contains("library data directory"));
    assert_eq!(
        editor.original.title, "Old title",
        "revert target is intact"
    );
}

#[test]
fn deliver_ignores_outcomes_for_other_files() {
    let track = track();
    let mut state = Some(TagEditorState::new(&track));

    deliver(
        &mut state,
        vec![EditOutcome {
            path: PathBuf::from("C:/music/other.flac"),
            result: Ok(()),
        }],
    );

    let editor = state.as_ref().expect("editor stays open");
    assert!(matches!(editor.status, Status::Editing));
}

#[test]
fn deliver_without_an_open_editor_is_a_no_op() {
    let mut state = None;
    deliver(
        &mut state,
        vec![EditOutcome {
            path: PathBuf::from("C:/music/track.flac"),
            result: Ok(()),
        }],
    );
    assert!(state.is_none());
}
