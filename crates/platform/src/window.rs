#![forbid(unsafe_code)]

//! An opaque native-window handle, the one value the app hands the shell.

/// The opaque native handle of a top-level window.
///
/// The app gets one from its backend (`Win32Backend::window_hwnd`) and passes
/// it to [`shell`](crate::shell); it is a plain integer so the seam stays free
/// of any backend or `windows` type, and the value is meaningful only to the
/// platform implementation that receives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHandle(isize);

impl NativeHandle {
    /// Wraps a backend's raw window handle.
    #[must_use]
    pub const fn from_raw(raw: isize) -> NativeHandle {
        NativeHandle(raw)
    }

    /// The raw value, for a platform implementation to interpret.
    #[must_use]
    pub const fn raw(self) -> isize {
        self.0
    }
}
