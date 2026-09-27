//! Modal and owned dialogs (#376): secondary `xui` windows that run their own
//! small [`App`](xui::xui_core::app::App) and hand a result back to the main
//! window, plus the shared text the in-window message dialogs show.

pub mod database_info;
pub mod properties;
pub mod tag_editor;

/// Extra height, in design units, a dialog adds to its content when it asks for
/// a window size.
///
/// `PlatformSpec::size` is the *outer* window size on the Win32 backend, so a
/// System-decorated dialog that lays its content out against the client rect
/// would otherwise be its caption and borders too short and the trailing button
/// row would collide with the last content row. The frame is roughly constant
/// in design units (it scales with the DPI), so one allowance covers every
/// scale; a backend that sizes by client area simply gets a little extra space
/// at the bottom.
pub const FRAME_ALLOWANCE: f32 = 48.0;

/// The Help -> Keyboard shortcuts text, rendered from the shared table so it
/// never drifts from the registered bindings.
pub fn shortcuts_text() -> String {
    let mut text = String::new();
    for shortcut in emusic_ui::state::SHORTCUTS {
        text.push_str(shortcut.display().as_str());
        text.push_str(" \u{2014} ");
        text.push_str(shortcut.description);
        text.push('\n');
    }
    text
}
