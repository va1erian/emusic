//! The file and folder dialogs, behind a seam.
//!
//! Every dialog in the app goes through [`pick_folder`], [`pick_file`] and
//! [`save_file`]. Each blocks until the user answers, so callers run them on a
//! background thread (see [`crate::folder_picker`]). By default they use the
//! platform dialogs through `rfd` (the `rfd` feature); a host without them
//! (LazyOS has no GTK or portal) builds without that feature and either
//! [`install`]s its own [`FilePicker`] or gets none, in which case every
//! dialog answers "cancelled".

use std::path::PathBuf;
use std::sync::OnceLock;

/// The file types a file dialog offers.
#[derive(Debug, Clone, Copy)]
pub struct Filter<'a> {
    /// What the types are called ("M3U playlist").
    pub name: &'a str,
    /// Their extensions, without the dot.
    pub extensions: &'a [&'a str],
}

/// Something that can show the app's file dialogs.
pub trait FilePicker: Send + Sync {
    /// Asks for an existing folder.
    fn pick_folder(&self) -> Option<PathBuf>;
    /// Asks for an existing file of the `filter` types.
    fn pick_file(&self, filter: Filter<'_>) -> Option<PathBuf>;
    /// Asks where to save a file of the `filter` types, suggesting
    /// `file_name`.
    fn save_file(&self, filter: Filter<'_>, file_name: &str) -> Option<PathBuf>;
}

static INSTALLED: OnceLock<Box<dyn FilePicker>> = OnceLock::new();

/// Makes `picker` show every dialog from now on. Returns false (and keeps the
/// current one) when a picker was installed already.
pub fn install(picker: Box<dyn FilePicker>) -> bool {
    INSTALLED.set(picker).is_ok()
}

/// Asks for an existing folder; `None` when cancelled or unavailable.
pub fn pick_folder() -> Option<PathBuf> {
    match INSTALLED.get() {
        Some(picker) => picker.pick_folder(),
        None => platform::pick_folder(),
    }
}

/// Asks for an existing file of the `filter` types.
pub fn pick_file(filter: Filter<'_>) -> Option<PathBuf> {
    match INSTALLED.get() {
        Some(picker) => picker.pick_file(filter),
        None => platform::pick_file(filter),
    }
}

/// Asks where to save a file of the `filter` types, suggesting `file_name`.
pub fn save_file(filter: Filter<'_>, file_name: &str) -> Option<PathBuf> {
    match INSTALLED.get() {
        Some(picker) => picker.save_file(filter, file_name),
        None => platform::save_file(filter, file_name),
    }
}

/// The platform dialogs, through `rfd`.
#[cfg(feature = "rfd")]
mod platform {
    use std::path::PathBuf;

    use super::Filter;

    pub(super) fn pick_folder() -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }

    pub(super) fn pick_file(filter: Filter<'_>) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(filter.name, filter.extensions)
            .pick_file()
    }

    pub(super) fn save_file(filter: Filter<'_>, file_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(filter.name, filter.extensions)
            .set_file_name(file_name)
            .save_file()
    }
}

/// No platform dialogs: every request is cancelled.
#[cfg(not(feature = "rfd"))]
mod platform {
    use std::path::PathBuf;

    use super::Filter;

    pub(super) fn pick_folder() -> Option<PathBuf> {
        tracing::warn!("no file dialog on this platform");
        None
    }

    pub(super) fn pick_file(_filter: Filter<'_>) -> Option<PathBuf> {
        pick_folder()
    }

    pub(super) fn save_file(_filter: Filter<'_>, _file_name: &str) -> Option<PathBuf> {
        pick_folder()
    }
}
