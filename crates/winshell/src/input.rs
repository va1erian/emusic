#![forbid(unsafe_code)]

//! Keyboard-focus queries used to keep bare-key shortcuts from stealing keys
//! from the controls that need them (#28).
//!
//! The native window registers accelerators on itself and exposes no focus
//! query, so the app asks here before acting on a bare-key binding: a focused
//! text field or navigation control consumes `Space`, the arrow keys, `Delete`
//! and friends on its own.

use crate::sys;

/// Whether the control that currently has keyboard focus consumes typing keys
/// (`Space`, the arrows, `Delete`, ...) itself, so a bare-key shortcut must be
/// ignored while it is focused.
///
/// True for the common controls that edit text or take arrow/tab navigation:
/// `Edit`, `ComboBox`, `Button`, `msctls_trackbar32` and `SysTabControl32`.
/// Only bare-key bindings are affected; Ctrl-bearing ones are safe to fire even
/// while typing, and the caller decides.
pub fn focused_control_consumes_keys() -> bool {
    sys::focused_window_class().is_some_and(|class| consumes_keys(&class))
}

/// A bare typing key a focused text or navigation control consumes on its
/// own, for the shortcut handler to give back after the accelerator table
/// has swallowed the physical keystroke while that control had focus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypingKey {
    /// A printable character (e.g. Space): `WM_KEYDOWN`, `WM_CHAR`,
    /// `WM_KEYUP`. Text controls insert the character on the `WM_CHAR`; a
    /// button activates on the `WM_KEYUP`.
    Char {
        /// The virtual key, for the surrounding key messages.
        vk: u16,
        /// The character the control receives.
        ch: char,
    },
    /// A navigation key such as an arrow: `WM_KEYDOWN` + `WM_KEYUP`.
    Key(u16),
}

/// Re-delivers the synthetic keystroke for `key` to the window that currently
/// has keyboard focus, so a text field still gets its space or an arrow
/// still moves its caret after the accelerator table consumed the physical
/// key while that field had focus.
///
/// The keystroke is sent directly to the focused window, bypassing the
/// message queue: a queued message would be offered to the accelerator table
/// again and consumed, re-raising the shortcut forever. Returns `false` when
/// no window has focus (e.g. the app is not foreground).
pub fn forward_typing_key(key: TypingKey) -> bool {
    let Some(hwnd) = sys::focused_window() else {
        return false;
    };
    sys::send_typing_key(key, hwnd);
    true
}

/// Whether a window class consumes typing keys itself.
fn consumes_keys(class: &str) -> bool {
    const CONSUMING: &[&str] = &[
        "Edit",
        "ComboBox",
        "Button",
        "msctls_trackbar32",
        "SysTabControl32",
    ];
    CONSUMING
        .iter()
        .any(|name| class.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::consumes_keys;

    #[test]
    fn text_and_navigation_controls_consume_keys() {
        for class in [
            "Edit",
            "ComboBox",
            "Button",
            "msctls_trackbar32",
            "SysTabControl32",
        ] {
            assert!(consumes_keys(class), "{class} consumes keys");
        }
    }

    #[test]
    fn lists_and_unknown_classes_do_not_consume_keys() {
        assert!(!consumes_keys("SysListView32"));
        assert!(!consumes_keys("SysTreeView32"));
        assert!(!consumes_keys(""));
    }
}
