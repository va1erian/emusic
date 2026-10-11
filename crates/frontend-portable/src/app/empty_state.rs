//! The Music view's empty state: with no library folder and no track, a line
//! of guidance and an **Add music folder...** button over the empty table,
//! instead of a blank list.

use xui::xui_core::app::Ui;
use xui::xui_core::arrange::{button, label};
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, Label};

use super::Msg;
use crate::make::make;
use crate::views::settings::SettingsMsg;

/// Width of the guidance line, in design units.
const HINT_WIDTH: f32 = 420.0;
/// Height of the guidance line, in design units.
const HINT_HEIGHT: f32 = 24.0;
/// Width of the button, in design units.
const BUTTON_WIDTH: f32 = 190.0;
/// Height of the button, in design units.
const BUTTON_HEIGHT: f32 = 30.0;
/// Gap between the line and the button, in design units.
const GAP: f32 = 10.0;

/// The guidance and its button.
pub(super) struct EmptyState {
    ui: Ui<Msg>,
    hint: Label<Msg>,
    add: Button<Msg>,
}

impl EmptyState {
    /// Builds the (hidden) widgets. Created after the central views so they
    /// paint over the music table.
    pub(super) fn new(ui: &Ui<Msg>) -> Self {
        let hint = make(
            ui,
            label("Your library is empty. Add a folder with music to start."),
        )
        .expect("create empty-library hint");
        let add = make(ui, button("Add music folder..."))
            .expect("create empty-library button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::AddFolder)));
        ui.set_visible(hint.id(), false);
        ui.set_visible(add.id(), false);
        Self {
            ui: ui.clone(),
            hint,
            add,
        }
    }

    /// Centres the pair in `area` (the central area), in device pixels.
    pub(super) fn set_bounds(&self, ui: &Ui<Msg>, area: Rect) {
        let dpi = ui.dpi();
        let px = |value: f32| dip(value).to_px(dpi).value();
        // Never wider than the area, so a narrow window clips nothing.
        let hint_w = px(HINT_WIDTH).min(area.width());
        let button_w = px(BUTTON_WIDTH).min(area.width());
        let (hint_h, button_h) = (px(HINT_HEIGHT), px(BUTTON_HEIGHT));
        let total = hint_h + px(GAP) + button_h;
        let cx = area.left + area.width() / 2;
        // And never above it, so a short window keeps the pair off the header.
        let top = area.top + ((area.height() - total) / 2).max(0);
        ui.apply_moves(&[
            (
                self.hint.id(),
                Rect::new(cx - hint_w / 2, top, cx + hint_w / 2, top + hint_h),
            ),
            (
                self.add.id(),
                Rect::new(
                    cx - button_w / 2,
                    top + hint_h + px(GAP),
                    cx + button_w / 2,
                    top + total,
                ),
            ),
        ]);
    }

    /// Shows or hides the pair.
    pub(super) fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.hint.id(), visible);
        self.ui.set_visible(self.add.id(), visible);
    }
}
