//! Remote-drive detection for library folder watching.
//!
//! Network roots (`\\server\share` or mapped drives that resolve to
//! `DRIVE_REMOTE`) are unreliable with `notify`/`ReadDirectoryChangesW`, so
//! the watcher uses periodic polling for them instead.

use std::path::Path;

/// Returns whether `path` should be polled periodically instead of watched
/// with `notify`.
///
/// UNC paths and mapped drives that Windows reports as `DRIVE_REMOTE` are
/// considered remote. Local paths and unrecognised prefixes are not.
pub fn is_remote_root(path: &Path) -> bool {
    winshell::is_remote_drive(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn unc_path_is_remote() {
        assert!(is_remote_root(Path::new(r"\\nas\music")));
        assert!(is_remote_root(Path::new(r"\\nas\music\artist")));
    }

    #[test]
    fn verbatim_unc_path_is_remote() {
        assert!(is_remote_root(Path::new(r"\\?\UNC\nas\music")));
    }

    #[test]
    fn local_drive_is_not_remote() {
        // The current test drive is local; GetDriveTypeW must agree.
        let local = PathBuf::from(std::env::var_os("SystemDrive").unwrap_or_else(|| "C:".into()))
            .join("windows");
        assert!(!is_remote_root(&local));
    }
}
