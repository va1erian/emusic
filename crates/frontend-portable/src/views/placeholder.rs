//! A documented placeholder for a view that is not ported yet.
//!
//! #370 established the portable shell and ported the Music view + track table;
//! #373 the navigator and Folders; #372 the list views (Artists, Genres,
//! Starred, Most Played, History, column browser). The remaining central views
//! are owned by later issues (#371 custom-painted views, #374 album grid,
//! #376 dialogs/settings). Until then each draws a single label naming the
//! view, so the shell stays green and every view has a stable place to grow
//! into.

use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::{HasText, Label};

use crate::app::Msg;

/// A not-yet-ported view: one label announcing what belongs here.
pub struct Placeholder {
    ui: Ui<Msg>,
    label: Label<Msg>,
}

impl Placeholder {
    /// Creates a placeholder for `view`, with a note pointing at its issue.
    pub fn new(ui: &Ui<Msg>, view: &str, owner: &str) -> Placeholder {
        let text = format!("{view}: not ported to xui_core yet ({owner})");
        let label = Label::new(ui, Rect::default(), &text).expect("create placeholder label");
        Placeholder {
            ui: ui.clone(),
            label,
        }
    }

    /// Moves/resizes the placeholder.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.label.id(), rect)]);
    }

    /// Shows or hides the placeholder.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.label.id(), visible);
    }

    /// Replaces the placeholder text.
    pub fn sync(&self, text: &str) {
        self.label.set_text(text);
    }
}
