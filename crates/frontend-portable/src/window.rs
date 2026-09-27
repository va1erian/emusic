//! The main window's portable spec, shared by the real binary and the
//! screenshot tool so the captured chrome cannot drift from what users get.
//!
//! The spec is portable; the window-level operations the widget layer does not
//! surface (the caption drag region and the window buttons) go through
//! [`WindowChrome`], which wraps the backend with the app's one window.

use std::rc::Rc;

use xui::xui_core::Dip;
use xui::xui_core::backend::{Backdrop, Backend, Decorations, PlatformSpec, WidgetId, WindowId};
use xui::xui_core::dip;

/// The height of the custom caption band reserved at the top of the client
/// area, in device-independent pixels.
pub const CAPTION_HEIGHT: f32 = 36.0;

/// Builds the main window spec: no system title bar (the shell reserves a
/// caption band and drags from it), with the Acrylic backdrop. The theme is
/// applied separately with `Ui::set_theme`, since the portable spec carries no
/// accent tint or menu strip (see the migration notes).
///
/// `width` and `height` are in device-independent pixels.
#[must_use]
pub fn window_spec(width: f32, height: f32) -> PlatformSpec {
    PlatformSpec::new("emusic")
        .size(dip(width), dip(height))
        .backdrop(Backdrop::Acrylic)
        .decorations(Decorations::None)
        .caption_inset(dip(CAPTION_HEIGHT))
}

/// The window-level operations the portable widget layer does not expose: the
/// caption drag region and the window buttons. It wraps the backend with the
/// app's single window, so the app never names a backend type.
pub struct WindowChrome {
    backend: Rc<dyn Backend>,
    window: WindowId,
}

impl WindowChrome {
    /// Wraps `backend` for `window`.
    pub fn new(backend: Rc<dyn Backend>, window: WindowId) -> WindowChrome {
        WindowChrome { backend, window }
    }

    /// Marks node `id`'s area as a window-drag region: a left press that starts
    /// there moves the window instead of reaching the widget.
    pub fn set_drag_region(&self, id: WidgetId, drag: bool) {
        self.backend.set_drag_region(id, drag);
    }

    /// Minimizes the window to the taskbar.
    pub fn minimize(&self) {
        self.backend.minimize(self.window);
    }

    /// Toggles the window between maximized and its restored bounds.
    pub fn toggle_maximize(&self) {
        self.backend.toggle_maximize(self.window);
    }

    /// Whether the window is currently maximized.
    pub fn is_maximized(&self) -> bool {
        self.backend.is_maximized(self.window)
    }

    /// The height the backend reserves at the top of the client area for the
    /// caption (the native strip on Windows, the requested band elsewhere).
    pub fn caption_inset(&self) -> Dip {
        self.backend.caption_inset(self.window)
    }
}
