//! The left navigator (#104), ported to the portable [`ListView`]: one row per
//! central view, mapping a selection to [`Msg::Navigate`](crate::app::Msg).
//!
//! The full navigator (#373) will grow a model-driven `TreeView`; this is the
//! minimal placeholder the portable shell needs to switch views.

use emusic_ui::state::View;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::ListView;

use crate::app::Msg;

/// The left navigator: the central-view switcher.
pub struct NavigatorView {
    ui: Ui<Msg>,
    list: ListView<Msg>,
}

impl NavigatorView {
    /// Creates the navigator with one row per central view.
    pub fn new(ui: &Ui<Msg>) -> NavigatorView {
        let labels: Vec<&str> = View::ALL.iter().map(|view| view.label()).collect();
        let list = ListView::new(ui, Rect::default(), &labels)
            .expect("create navigator list")
            .on_select(|row| View::ALL.get(row).copied().map(Msg::Navigate));
        NavigatorView {
            ui: ui.clone(),
            list,
        }
    }

    /// Moves/resizes the navigator.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.list.id(), rect)]);
    }

    /// Shows or hides the navigator.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.list.id(), visible);
    }

    /// Selects the row for the active view without raising an event.
    pub fn sync(&self, view: View) {
        let index = View::ALL.iter().position(|candidate| *candidate == view);
        self.list.select(index);
    }
}
