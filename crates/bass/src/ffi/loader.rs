//! Locating and `dlopen`-ing the BASS library (and its plugins) at runtime.
//!
//! BASS binaries are never committed to this repository and this crate has no
//! link-time dependency on them: everything is resolved dynamically with
//! [`libloading`] so the workspace builds and tests fine on a machine that
//! doesn't have BASS installed at all. The file names follow each platform's
//! convention: `bass.dll`/`bassmidi.dll` on Windows, `libbass.dylib`/
//! `libbassmidi.dylib` on macOS (and `libbass.so` elsewhere).

use std::env;
use std::path::{Path, PathBuf};

use libloading::Library;

use crate::error::BassError;

/// Environment variable that overrides where BASS is looked for.
pub const BASS_DIR_ENV: &str = "EMUSIC_BASS_DIR";

/// The core BASS library's file name on this platform.
#[cfg(windows)]
pub const CORE_LIBRARY: &str = "bass.dll";
/// The core BASS library's file name on this platform.
#[cfg(target_os = "macos")]
pub const CORE_LIBRARY: &str = "libbass.dylib";
/// The core BASS library's file name on this platform.
#[cfg(all(not(windows), not(target_os = "macos")))]
pub const CORE_LIBRARY: &str = "libbass.so";

/// The `bassmidi` add-on's file name on this platform.
#[cfg(windows)]
pub const MIDI_LIBRARY: &str = "bassmidi.dll";
/// The `bassmidi` add-on's file name on this platform.
#[cfg(target_os = "macos")]
pub const MIDI_LIBRARY: &str = "libbassmidi.dylib";
/// The `bassmidi` add-on's file name on this platform.
#[cfg(all(not(windows), not(target_os = "macos")))]
pub const MIDI_LIBRARY: &str = "libbassmidi.so";

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

/// Loads a BASS library by file name (e.g. `CORE_LIBRARY`) from [`bass_dir`].
///
/// Returns [`BassError::DllNotFound`] if the directory or file doesn't
/// exist, or if the OS loader rejects the file (missing dependency, wrong
/// architecture, etc).
pub fn load_dll(file_name: &str) -> Result<Library, BassError> {
    let dir = bass_dir()?;
    load_dll_from(&dir, file_name)
}

/// Loads a BASS library by file name from a specific directory.
pub fn load_dll_from(dir: &Path, file_name: &str) -> Result<Library, BassError> {
    let path = dir.join(file_name);
    if !path.is_file() {
        return Err(BassError::DllNotFound(path.display().to_string()));
    }
    // SAFETY: `libloading::Library::new` runs the library's initialiser,
    // which is inherently unsafe (arbitrary code we don't control). We
    // mitigate this by only ever loading BASS library file names
    // (`bass*.dll` / `libbass*.dylib`) from a directory the user/deployment
    // explicitly configured (`EMUSIC_BASS_DIR` or the install's `bass/`
    // folder), never an arbitrary or attacker-controlled path.
    unsafe { Library::new(&path) }.map_err(|e| BassError::DllNotFound(format!("{path:?}: {e}")))
}

/// Lists every BASS add-on/plugin library in `dir` other than the core one —
/// e.g. `bassflac.dll`/`bassopus.dll` on Windows, `libbassflac.dylib`/
/// `libbassopus.dylib` on macOS.
pub fn list_plugin_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut plugins: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_plugin_name)
        })
        .collect();
    plugins.sort();
    plugins
}

/// Whether `name` looks like a BASS add-on library rather than the core one on
/// this platform.
fn is_plugin_name(name: &str) -> bool {
    if name.eq_ignore_ascii_case(CORE_LIBRARY) {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    if cfg!(windows) {
        lower.starts_with("bass") && lower.ends_with(".dll")
    } else if cfg!(target_os = "macos") {
        lower.starts_with("libbass") && lower.ends_with(".dylib")
    } else {
        lower.starts_with("libbass") && lower.ends_with(".so")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_library_is_not_a_plugin() {
        assert!(!is_plugin_name(CORE_LIBRARY));
        assert!(!is_plugin_name(&CORE_LIBRARY.to_ascii_uppercase()));
    }

    #[test]
    fn add_on_names_are_plugins() {
        #[cfg(windows)]
        {
            assert!(is_plugin_name("bassflac.dll"));
            assert!(is_plugin_name("BASSOPUS.DLL"));
        }
        #[cfg(target_os = "macos")]
        {
            assert!(is_plugin_name("libbassflac.dylib"));
            assert!(is_plugin_name("libbassopus.dylib"));
        }
        #[cfg(all(not(windows), not(target_os = "macos")))]
        {
            assert!(is_plugin_name("libbassflac.so"));
            assert!(is_plugin_name("libbassopus.so"));
        }
    }

    #[test]
    fn other_files_are_not_plugins() {
        assert!(!is_plugin_name("readme.txt"));
        assert!(!is_plugin_name("libbass.txt"));
    }
}
