//! The main window's portable spec, shared by the real binary and the
//! screenshot tool so the captured chrome cannot drift from what users get.

use xui::xui_core::backend::{Backdrop, Decorations, PlatformSpec};
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
