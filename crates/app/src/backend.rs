//! The rendering backend this process is running on (#369).
//!
//! The binary picks the backend at launch (native `win32` on Windows by
//! default, the software `canvas` backend elsewhere or when `XUI_BACKEND=canvas`
//! requests it). The About page shows which one is active; the same predicate
//! is what [`crate`]'s `main` uses to choose, so the two cannot drift.

/// Whether the software `canvas` backend is active.
///
/// `XUI_BACKEND=canvas` requests it. On a build without the `canvas` feature
/// the request cannot be honoured, so it is ignored (with a warning) and the
/// native backend is used; off Windows, `canvas` is the only backend.
pub fn is_canvas() -> bool {
    if !cfg!(windows) {
        return true;
    }
    let requested = std::env::var("XUI_BACKEND").as_deref() == Ok("canvas");
    if requested && !cfg!(feature = "canvas") {
        tracing::warn!(
            "XUI_BACKEND=canvas but this build has no `canvas` feature; using the native backend"
        );
        return false;
    }
    requested
}

/// A short human-readable name for the active backend, for the About page.
pub fn label() -> &'static str {
    if is_canvas() {
        "canvas (software)"
    } else {
        "Win32"
    }
}
