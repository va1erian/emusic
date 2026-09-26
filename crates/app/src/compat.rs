//! Temporary shims for `xui` APIs emusic relies on but that the pinned `xui`
//! rev does not expose yet.
//!
//! The accent-tinted material bands (emusic #355) landed in `win32ui` (the
//! `Accent-tint the material bands` commit) *after* the fork that became
//! `xui-win32`, and the pinned `xui` rev does not carry that port. The
//! `WindowSpec`/`Ui` methods that turn the tint on therefore no-op here, so the
//! dependency swap and the `#[cfg]` seam (#370) can land without dropping the
//! call sites. The settings still round-trip through [`emusic_ui`]; only the
//! visual tint is inert until xui gains the feature.
//!
//! Delete this module (and its imports) once `xui` exposes
//! `WindowSpec::accent_tint{,_strength}` and `Ui::set_accent_tint{,_strength}`.

use xui::{Ui, WindowSpec};

/// The builder methods `crate::window::window_spec` chains on a [`WindowSpec`].
pub trait WindowSpecAccentTint {
    /// Requests the accent-tinted material bands. No-op on the pinned `xui`.
    fn accent_tint(self, on: bool) -> Self;

    /// Sets the tint strength (0-255). No-op on the pinned `xui`.
    fn accent_tint_strength(self, strength: u8) -> Self;
}

impl WindowSpecAccentTint for WindowSpec {
    fn accent_tint(self, _on: bool) -> Self {
        self
    }

    fn accent_tint_strength(self, _strength: u8) -> Self {
        self
    }
}

/// The live setters `Win32App` calls when the appearance changes.
pub trait UiAccentTint {
    /// Flips the accent tint live. No-op on the pinned `xui`.
    fn set_accent_tint(&self, on: bool);

    /// Updates the tint strength live. No-op on the pinned `xui`.
    fn set_accent_tint_strength(&self, strength: u8);
}

impl<M> UiAccentTint for Ui<M> {
    fn set_accent_tint(&self, _on: bool) {}

    fn set_accent_tint_strength(&self, _strength: u8) {}
}
