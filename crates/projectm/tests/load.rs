//! Loads the real projectM DLLs when they are available (`EMUSIC_PROJECTM_DIR`
//! or `projectm\` next to the test binary) and skips otherwise, like the
//! BASS tests. Rendering needs an OpenGL context and is exercised by the
//! frontend instead.

#![forbid(unsafe_code)]

use emusic_projectm::{ProjectM, ProjectMError};

/// The loaded libraries, or `None` (with a note) when they are absent.
fn projectm() -> Option<ProjectM> {
    match ProjectM::load() {
        Ok(projectm) => Some(projectm),
        Err(err) => {
            eprintln!("skipping: projectM not available ({err})");
            None
        }
    }
}

#[test]
fn reports_a_4_x_version() {
    let Some(projectm) = projectm() else { return };
    let version = projectm.version();
    assert!(version.starts_with('4'), "unexpected version {version:?}");
}

#[test]
fn refuses_to_create_without_a_current_context() {
    let Some(projectm) = projectm() else { return };
    assert!(!projectm.has_current_context());
    let err = projectm.create(640, 480).expect_err("no GL context here");
    assert!(matches!(err, ProjectMError::NoCurrentContext), "{err:?}");
}
