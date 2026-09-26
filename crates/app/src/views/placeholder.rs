//! Placeholder views (#106).
//!
//! Until the real Win32 views land (#107–#115), each not-yet-implemented
//! region draws a single label. The shape is the one every real view will
//! follow: a struct owning its control(s), a `sync` that updates it from the
//! shell's model, and an [`AsControl`] impl so the layout tree can place it.

use xui::prelude::*;
use xui::{Control, Label};

/// A not-yet-implemented view: one static label.
pub struct Placeholder {
    label: Label,
}

impl Placeholder {
    /// Creates a placeholder label for a region.
    pub fn new<M: 'static>(ui: &mut Ui<M>, text: &str) -> Result<Self> {
        Ok(Self {
            label: Label::new(ui, Rect::default(), text)?,
        })
    }

    /// Updates the label's text.
    pub fn sync(&self, text: &str) {
        self.label.set_text(text);
    }
}

impl AsControl for Placeholder {
    fn control(&self) -> &Control {
        self.label.control()
    }
}
