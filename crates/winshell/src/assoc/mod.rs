//! Per-user file association registration (`HKCU`, no admin rights).

mod manager;
mod roots;

use std::io;

pub use manager::{AssocManager, EXTENSIONS};
pub use roots::AssocRoots;

/// Opens the Windows Settings page for choosing default apps, scrolled to
/// emusic (`ms-settings:defaultapps?registeredAppUser=<name>`).
///
/// Windows 10/11 do not let an app force itself as the default handler; the
/// best an app can do is register (see [`AssocManager::register`]) and then
/// point the user here.
pub fn open_default_apps_settings(app_name: &str) -> io::Result<()> {
    crate::sys::shell_open(&format!(
        "ms-settings:defaultapps?registeredAppUser={app_name}"
    ))
}
