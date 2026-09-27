#![forbid(unsafe_code)]

//! File associations for Windows, on top of `winshell`.
//!
//! Per-user registration under `HKCU\Software\Classes` (no elevation): a
//! ProgID with `shell\open\command`, the extension keys' `OpenWithProgids`,
//! and an `Applications\<exe>` entry. `winshell` refreshes the shell caches.

/// Registers this process as the handler for emusic's file extensions.
pub(crate) fn register() -> Result<(), String> {
    let exe =
        std::env::current_exe().map_err(|err| format!("could not locate the executable: {err}"))?;
    let manager = winshell::assoc::AssocManager::new("emusic");
    manager
        .register(&exe, winshell::assoc::EXTENSIONS)
        .map_err(|err| err.to_string())
}

/// Removes the associations registered by [`register`].
pub(crate) fn unregister() -> Result<(), String> {
    let manager = winshell::assoc::AssocManager::new("emusic");
    manager.unregister().map_err(|err| err.to_string())
}
