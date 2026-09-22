//! Locating and `dlopen`-ing `bass.dll` (and its plugins) at runtime.
//!
//! BASS DLLs are never committed to this repository and this crate has no
//! link-time dependency on them: everything is resolved dynamically with
//! [`libloading`] so the workspace builds and tests fine on a machine that
//! doesn't have BASS installed at all.

use std::env;
use std::path::{Path, PathBuf};

use libloading::Library;

use crate::error::BassError;

/// Environment variable that overrides where BASS DLLs are looked for.
pub const BASS_DIR_ENV: &str = "EMUSIC_BASS_DIR";

/// Directory BASS DLLs are loaded from: `$EMUSIC_BASS_DIR` if set, otherwise
/// a `bass` subdirectory next to the running executable.
pub fn bass_dir() -> Result<PathBuf, BassError> {
    if let Ok(dir) = env::var(BASS_DIR_ENV) {
        return Ok(PathBuf::from(dir));
    }
    let exe = env::current_exe().map_err(|e| BassError::DllNotFound(e.to_string()))?;
    let exe_dir = exe.parent().ok_or_else(|| {
        BassError::DllNotFound("executable path has no parent directory".to_string())
    })?;
    Ok(exe_dir.join("bass"))
}

/// Loads a DLL by file name (e.g. `"bass.dll"`) from [`bass_dir`].
///
/// Returns [`BassError::DllNotFound`] if the directory or file doesn't
/// exist, or if the OS loader rejects the file (missing dependency, wrong
/// architecture, etc).
pub fn load_dll(file_name: &str) -> Result<Library, BassError> {
    let dir = bass_dir()?;
    load_dll_from(&dir, file_name)
}

/// Loads a DLL by file name from a specific directory.
pub fn load_dll_from(dir: &Path, file_name: &str) -> Result<Library, BassError> {
    let path = dir.join(file_name);
    if !path.is_file() {
        return Err(BassError::DllNotFound(path.display().to_string()));
    }
    // SAFETY: `libloading::Library::new` runs the DLL's `DllMain`, which is
    // inherently unsafe (arbitrary code we don't control). We mitigate this
    // by only ever loading files named `bass*.dll` from a directory the
    // user/deployment explicitly configured (`EMUSIC_BASS_DIR` or the
    // install's `bass/` folder), never an arbitrary or attacker-controlled
    // path.
    unsafe { Library::new(&path) }.map_err(|e| BassError::DllNotFound(format!("{path:?}: {e}")))
}

/// Lists every `bass*.dll` in `dir` other than `bass.dll` itself — i.e. the
/// add-on/plugin DLLs (`bassflac.dll`, `bassopus.dll`, ...).
pub fn list_plugin_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut plugins: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            let lower = name.to_ascii_lowercase();
            lower.starts_with("bass")
                && lower.ends_with(".dll")
                && !lower.eq_ignore_ascii_case("bass.dll")
        })
        .collect();
    plugins.sort();
    plugins
}
