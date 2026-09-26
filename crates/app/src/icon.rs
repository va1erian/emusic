#![forbid(unsafe_code)]

//! The app's embedded window icon (`emusic.rc`, resource id 1, built by
//! `build.rs` from `assets/ico/emusic-app.ico`).
//!
//! win32ui registers the window class with `hIcon`/`hIconSm` unset, so a
//! freshly created top-level window starts with the generic system icon. Every
//! top-level window the app opens installs the embedded icon through
//! [`install`], so the title bar, Alt+Tab and the taskbar hover preview all
//! show emusic rather than the generic icon.

use win32ui::prelude::*;

/// The resource id of the icon embedded from `emusic.rc`.
const ICON_RESOURCE_ID: u16 = 1;

/// Installs the embedded app icon as `ui`'s large and small icon.
///
/// A missing or unloadable resource is ignored: the window keeps the system
/// default icon instead of failing to open.
pub fn install<M: 'static>(ui: &Ui<M>) {
    if let Ok(icon) = Icon::from_resource(ICON_RESOURCE_ID) {
        ui.set_icon(icon);
    }
}
