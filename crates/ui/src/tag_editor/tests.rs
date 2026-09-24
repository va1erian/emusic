//! Unit tests for the tag editor's form conversion and result handling.

use std::path::PathBuf;

use emusic_library::LibraryError;

use super::form::TagForm;
use super::{AutoTagState, Status, TagEditorState, deliver, deliver_auto_tag};
use crate::library_api::{AutoTagError, AutoTagOutcome, Candidate, EditOutcome, TrackInfo};

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

fn candidate(title: &str) -> Candidate {
    Candidate {
        title: Some(title.to_string()),
        score: 0.9,
        ..Default::default()
    }
}

fn auto_tag_outcome(result: Result<Vec<Candidate>, AutoTagError>) -> AutoTagOutcome {
    AutoTagOutcome {
        path: PathBuf::from("C:/music/track.flac"),
        result,
    }
}

#[test]
fn deliver_auto_tag_stores_candidates() {
    let track = track();
    let mut state = Some(TagEditorState::new(&track));
    state.as_mut().expect("editor is open").auto_tag = AutoTagState::Searching;

    deliver_auto_tag(
        &mut state,
        vec![auto_tag_outcome(Ok(vec![candidate("Found")]))],
    );

    let editor = state.as_ref().expect("editor stays open");
    let AutoTagState::Matches(candidates) = &editor.auto_tag else {
        panic!("expected matches");
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title.as_deref(), Some("Found"));
}

#[test]
fn deliver_auto_tag_without_candidates_is_no_match() {
    let track = track();
    let mut state = Some(TagEditorState::new(&track));

    deliver_auto_tag(&mut state, vec![auto_tag_outcome(Ok(Vec::new()))]);

    let editor = state.as_ref().expect("editor stays open");
    assert!(matches!(editor.auto_tag, AutoTagState::NoMatch));
}

#[test]
fn deliver_auto_tag_failure_shows_the_message() {
    let track = track();
    let mut state = Some(TagEditorState::new(&track));

    deliver_auto_tag(
        &mut state,
        vec![auto_tag_outcome(Err(AutoTagError::Provider(
            emusic_metadata::MetadataError::NoMatch,
        )))],
    );

    let editor = state.as_ref().expect("editor stays open");
    let AutoTagState::Failed(message) = &editor.auto_tag else {
        panic!("expected a failure");
    };
    assert!(message.contains("no matching metadata"));
}

#[test]
fn deliver_auto_tag_cancelled_returns_to_idle() {
    let track = track();
    let mut state = Some(TagEditorState::new(&track));
    state.as_mut().expect("editor is open").auto_tag = AutoTagState::Searching;

    deliver_auto_tag(
        &mut state,
        vec![auto_tag_outcome(Err(AutoTagError::Cancelled))],
    );

    let editor = state.as_ref().expect("editor stays open");
    assert!(matches!(editor.auto_tag, AutoTagState::Idle));
}

#[test]
fn deliver_auto_tag_ignores_other_files() {
    let track = track();
    let mut state = Some(TagEditorState::new(&track));

    deliver_auto_tag(
        &mut state,
        vec![AutoTagOutcome {
            path: PathBuf::from("C:/music/other.flac"),
            result: Ok(vec![candidate("Found")]),
        }],
    );

    let editor = state.as_ref().expect("editor stays open");
    assert!(matches!(editor.auto_tag, AutoTagState::Idle));
}
