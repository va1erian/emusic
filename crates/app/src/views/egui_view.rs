//! egui renderers for the toolkit-agnostic view models (#98).
//!
//! A view model owns its state and logic in `emusic-ui`; the egui side only
//! draws it, turns widget responses into the model's messages, and lets the
//! model's `update` apply them. Keeping the seam to one trait makes it
//! obvious which code is render-only.

use eframe::egui;

use emusic_ui::views::{Commands, Ctx};

/// An egui renderer for a view model.
pub(crate) trait EguiView {
    /// Draws the view for this frame, applying the messages its widgets
    /// produced to the model and queueing any resulting commands into `out`.
    ///
    /// `id_salt` disambiguates egui's per-widget state when the same view is
    /// shown more than once (e.g. `emusic-shot --all`).
    fn show(&mut self, ui: &mut egui::Ui, id_salt: &str, cx: &Ctx, out: &mut Commands);
}
