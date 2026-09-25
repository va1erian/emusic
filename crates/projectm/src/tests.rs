#![forbid(unsafe_code)]

use std::path::PathBuf;

use crate::{CORE_DLL, ProjectM, ProjectMError};

/// A directory that exists but holds no DLLs.
fn empty_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emusic-projectm-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn missing_folder_reports_dll_not_found() {
    let dir = std::env::temp_dir().join("emusic-projectm-does-not-exist");
    let err = ProjectM::load_from(&dir).expect_err("nothing to load");
    assert!(matches!(err, ProjectMError::DllNotFound(_)), "{err:?}");
}

#[test]
fn folder_without_the_core_dll_names_it() {
    let dir = empty_dir("no-core");
    match ProjectM::load_from(&dir) {
        Err(ProjectMError::DllNotFound(what)) => assert!(what.contains(CORE_DLL), "{what}"),
        other => panic!("expected DllNotFound, got {other:?}"),
    }
    std::fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn a_file_that_is_not_a_library_is_rejected() {
    let dir = empty_dir("bogus");
    std::fs::write(dir.join(CORE_DLL), b"not a dll").expect("write bogus dll");
    let err = ProjectM::load_from(&dir).expect_err("bogus file");
    assert!(matches!(err, ProjectMError::DllNotFound(_)), "{err:?}");
    std::fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn errors_read_well() {
    assert_eq!(
        ProjectMError::MissingSymbol("projectm_create".into()).to_string(),
        "projectM library is missing the `projectm_create` entry point"
    );
    assert_eq!(
        ProjectMError::NoCurrentContext.to_string(),
        "no OpenGL context is current on this thread"
    );
}
