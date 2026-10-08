//! The accent preset strip (#276, #376) on the portable [`ColorPicker`]: one
//! filled swatch per preset, the current accent ringed, a click picks it.

use emusic_ui::state::{Accent, Rgb};
use xui::xui_core::Color;
use xui::xui_core::app::Ui;
use xui::xui_core::widget::ColorPicker;

use crate::app::Msg;
use crate::make::make;
use xui::xui_core::arrange::color_picker;

/// Height of the swatch strip, in design units.
pub const STRIP_HEIGHT: f32 = 44.0;

/// The portable colour for a shared [`Rgb`].
pub fn color(rgb: Rgb) -> Color {
    let [r, g, b] = rgb.to_array();
    Color::rgb(r, g, b)
}

/// The picker's palette: one colour per accent preset, in display order.
fn palette() -> Vec<Color> {
    Accent::PRESETS
        .into_iter()
        .map(|a| color(a.rgb()))
        .collect()
}

/// Creates the picker; a click maps to `Msg::SetAccent`.
pub fn create(ui: &Ui<Msg>) -> ColorPicker<Msg> {
    make(ui, color_picker(&palette()))
        .expect("create accent picker")
        .columns(Accent::PRESETS.len())
        .on_select(|chosen| {
            Accent::PRESETS
                .into_iter()
                .find(|accent| color(accent.rgb()) == chosen)
                .map(Msg::SetAccent)
        })
}
