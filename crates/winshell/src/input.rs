#![forbid(unsafe_code)]

//! Keyboard-focus queries used to keep bare-key shortcuts from stealing keys
//! from the controls that need them (#28).
//!
//! `win32ui` registers accelerators on the window itself and exposes no focus
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
