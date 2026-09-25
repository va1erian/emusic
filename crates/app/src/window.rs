//! The main window's spec, shared by the real binary and the screenshot tool
//! so the captured chrome cannot drift from what users get.

use win32ui::prelude::*;

/// Builds the main window spec: an extended title bar carrying the caption,
/// the menu strip and the transport band (#108), with the Acrylic backdrop.
/// `width` and `height` are in DIPs.
pub fn window_spec(width: f32, height: f32, theme: win32ui::Theme) -> WindowSpec {
    WindowSpec::new("emusic")
        .size(dip(width), dip(height))
        .theme(theme)
        // The menu moves onto the strip so the top band sits below it.
        .title_bar(TitleBar::Extended)
        .backdrop(Backdrop::Acrylic)
        .menu_in_strip(true)
}
