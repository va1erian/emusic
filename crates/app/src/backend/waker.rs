//! egui's [`Waker`]: ask the window to repaint (#95).
//!
//! Background workers hold an [`emusic_ui::waker::WakerHandle`] rather than an
//! [`egui::Context`]; the shell binds one of these to the context once
//! `eframe` has created it, so a wake anywhere becomes
//! [`egui::Context::request_repaint`].

use eframe::egui;
use emusic_ui::waker::Waker;

/// Wakes the egui frontend by requesting a repaint.
pub struct EguiWaker(egui::Context);

impl EguiWaker {
    /// Wraps the context `eframe` hands the app on creation.
    #[must_use]
    pub fn new(ctx: egui::Context) -> Self {
        Self(ctx)
    }
}

impl Waker for EguiWaker {
    fn wake(&self) {
        self.0.request_repaint();
    }
}
