//! The rendering backend this process is running on (#369, #470).
//!
//! `xui`'s software `canvas` backend is this frontend's own backend, on every
//! target; a host that runs the app on its window system through
//! [`run_on`](crate::run_on) describes that one instead.

use std::sync::OnceLock;

/// What the host's window system is like, when a host set it.
struct HostChrome {
    native_chrome: bool,
    label: &'static str,
}

static HOST: OnceLock<HostChrome> = OnceLock::new();

/// Records the host's window system, before the window is built.
pub(crate) fn set_host(native_chrome: bool, label: &'static str) {
    let _ = HOST.set(HostChrome {
        native_chrome,
        label,
    });
}

/// Whether the window backend draws its own caption and window buttons, so
/// the app must not draw the custom caption band.
///
/// True for every macOS window (system decorations, per the macOS port) and
/// for a host that says so; false elsewhere, since the software canvas backend
/// has no native chrome and so needs the app's own.
pub fn has_native_chrome() -> bool {
    HOST.get()
        .map_or(cfg!(target_os = "macos"), |host| host.native_chrome)
}

/// A short human-readable name for the active backend, for the About page.
pub fn label() -> &'static str {
    HOST.get().map_or("canvas (software)", |host| host.label)
}
