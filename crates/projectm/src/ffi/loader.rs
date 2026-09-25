//! Locating and loading the projectM DLLs at runtime.
//!
//! Nothing is linked at build time: the DLLs are resolved with
//! [`libloading`], so the workspace builds and tests on a machine without
//! projectM, and a user can swap the LGPL libraries for their own build.

use std::env;
use std::path::{Path, PathBuf};

use libloading::Library;

use crate::error::ProjectMError;

/// Environment variable overriding where the projectM DLLs are looked for.
pub const PROJECTM_DIR_ENV: &str = "EMUSIC_PROJECTM_DIR";

/// The core library.
pub const CORE_DLL: &str = "projectM-4.dll";
/// The preset playlist library.
pub const PLAYLIST_DLL: &str = "projectM-4-playlist.dll";
/// The OpenGL loader projectM 4.1 links against on Windows.
pub const GLEW_DLL: &str = "glew32.dll";

/// `$EMUSIC_PROJECTM_DIR` if set, else a `projectm` folder next to the
/// running executable.
pub fn projectm_dir() -> Result<PathBuf, ProjectMError> {
    if let Ok(dir) = env::var(PROJECTM_DIR_ENV) {
        return Ok(PathBuf::from(dir));
    }
    let exe = env::current_exe().map_err(|e| ProjectMError::DllNotFound(e.to_string()))?;
    let exe_dir = exe.parent().ok_or_else(|| {
        ProjectMError::DllNotFound("executable path has no parent directory".to_string())
    })?;
    Ok(exe_dir.join("projectm"))
}

/// Loads `file_name` from `dir`. Its own dependencies (the playlist library
/// needs the core one, the core one needs GLEW and the MSVC runtime) are
/// searched in `dir` first, so the folder is self-contained.
pub fn load_from(dir: &Path, file_name: &str) -> Result<Library, ProjectMError> {
    let path = dir.join(file_name);
    if !path.is_file() {
        return Err(ProjectMError::DllNotFound(path.display().to_string()));
    }
    open(&path).map_err(|e| ProjectMError::DllNotFound(format!("{}: {e}", path.display())))
}

#[cfg(windows)]
fn open(path: &Path) -> Result<Library, libloading::Error> {
    use libloading::os::windows::{LOAD_WITH_ALTERED_SEARCH_PATH, Library as WinLibrary};
    // SAFETY: loading runs the DLL's `DllMain`, arbitrary code we don't
    // control. We only load the fixed projectM/GLEW file names from the
    // folder the deployment configured (`EMUSIC_PROJECTM_DIR` or the
    // install's `projectm\`), never an arbitrary path. The altered search
    // path makes the DLL's own imports resolve from that same folder.
    unsafe { WinLibrary::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) }.map(Library::from)
}

#[cfg(not(windows))]
fn open(path: &Path) -> Result<Library, libloading::Error> {
    // SAFETY: as on Windows, only fixed file names from the configured
    // folder are loaded; running their initialisers is the accepted risk.
    unsafe { Library::new(path) }
}

/// Loads a system library by name (e.g. `opengl32.dll`) through the normal
/// search order.
#[cfg(windows)]
pub fn load_system(file_name: &str) -> Result<Library, ProjectMError> {
    // SAFETY: `opengl32.dll` is a Windows system library; loading it runs
    // only its well-known initialiser.
    unsafe { Library::new(file_name) }
        .map_err(|e| ProjectMError::DllNotFound(format!("{file_name}: {e}")))
}
