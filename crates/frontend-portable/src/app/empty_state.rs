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
        let gap = px(GAP);
        let total = hint_h + gap + button_h;
        let cx = area.left + area.width() / 2;
        // Centred when the pair fits. When it does not, the button comes
        // first: it sits at the bottom of the area and the line above it is
        // collapsed rather than drawn over the table header.
        let (hint_top, button_top) = if total <= area.height() {
            let top = area.top + (area.height() - total) / 2;
            (top, top + hint_h + gap)
        } else {
            let button_top = (area.bottom - button_h).max(area.top);
            (button_top - gap - hint_h, button_top)
        };
        let hint_fits = hint_top >= area.top;
        let hint_rect = if hint_fits {
            Rect::new(
                cx - hint_w / 2,
                hint_top,
                cx + hint_w / 2,
                hint_top + hint_h,
            )
        } else {
            Rect::new(cx, area.top, cx, area.top)
        };
        ui.apply_moves(&[
            (self.hint.id(), hint_rect),
            (
                self.add.id(),
                Rect::new(
                    cx - button_w / 2,
                    button_top,
                    cx + button_w / 2,
                    (button_top + button_h).min(area.bottom.max(button_top)),
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
