//! The rendering backend this process is running on (#369, #470).
//!
//! `xui`'s software `canvas` backend is the only backend this frontend uses,
//! on every target.

/// Whether the window backend draws its own caption and window buttons, so
/// the app must not draw the custom caption band.
///
/// True for every macOS window (system decorations, per the macOS port);
/// false everywhere else, since the software canvas backend has no native
/// chrome and so needs the app's own.
pub fn has_native_chrome() -> bool {
    cfg!(target_os = "macos")
}

/// A short human-readable name for the active backend, for the About page.
pub fn label() -> &'static str {
    "canvas (software)"
}
