//! The accent preset strip (#276): one filled square per preset, the current
//! accent ringed, a click picks it.

use std::cell::Cell;

use emusic_ui::state::Accent;
use win32ui::gdi::Canvas;
use win32ui::prelude::*;
use win32ui::{Color, Custom, CustomWidget, Input, MouseButton, Size, Theme, WidgetCx, dip};

/// Side of one swatch, in design units.
const SWATCH: f32 = 24.0;
/// Gap between swatches, in design units.
const GAP: f32 = 10.0;
/// Thickness of the selection ring drawn outside a swatch, in design units.
const RING: f32 = 2.0;
/// Spare width so per-swatch pixel rounding never clips the last ring.
const SLACK: f32 = 4.0;
/// Width of the whole strip, in design units.
pub(super) const STRIP_WIDTH: f32 = Accent::PRESETS.len() as f32 * SWATCH
    + (Accent::PRESETS.len() - 1) as f32 * GAP
    + 2.0 * RING
    + SLACK;

/// The owner-drawn strip of preset swatches.
pub(super) struct AccentSwatches {
    selected: Cell<Accent>,
    dpi: Cell<u32>,
}

impl AccentSwatches {
    pub(super) fn new(dpi: u32) -> Self {
        Self {
            selected: Cell::new(Accent::default()),
            dpi: Cell::new(dpi),
        }
    }

    /// Marks `accent` as the current one; returns whether that changed it.
    pub(super) fn select(&self, accent: Accent) -> bool {
        self.selected.replace(accent) != accent
    }

    /// The pixel rectangle of swatch `index`, vertically centred in `bounds`.
    fn swatch_rect(&self, index: usize, bounds: Rect) -> Rect {
        let dpi = self.dpi.get();
        let side = dip(SWATCH).to_px(dpi).value();
        let step = side + dip(GAP).to_px(dpi).value();
        let left = bounds.left + dip(RING).to_px(dpi).value() + index as i32 * step;
        let top = bounds.top + (bounds.bottom - bounds.top - side) / 2;
        Rect::new(left, top, left + side, top + side)
    }

    fn hit(&self, x: i32, y: i32, bounds: Rect) -> Option<Accent> {
        Accent::PRESETS
            .into_iter()
            .enumerate()
            .find_map(|(i, accent)| {
                let rect = self.swatch_rect(i, bounds);
                (x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom)
                    .then_some(accent)
            })
    }
}

impl CustomWidget for AccentSwatches {
    type Event = Accent;

    fn preferred_size(&self, dpi: u32) -> Option<Size> {
        self.dpi.set(dpi);
        Some(Size::new(
            dip(STRIP_WIDTH).to_px(dpi).value(),
            dip(SWATCH + 2.0 * RING).to_px(dpi).value(),
        ))
    }

    fn paint(&self, canvas: &Canvas, bounds: Rect, theme: &Theme) {
        canvas.fill_rect(bounds, theme.background);
        let ring = dip(RING).to_px(self.dpi.get()).value();
        for (index, accent) in Accent::PRESETS.into_iter().enumerate() {
            let rect = self.swatch_rect(index, bounds);
            let [r, g, b] = accent.rgb().to_array();
            canvas.fill_rect(rect, Color::rgb(r, g, b));
            if accent == self.selected.get() {
                for inset in 0..ring {
                    let outer = Rect::new(
                        rect.left - ring + inset,
                        rect.top - ring + inset,
                        rect.right + ring - inset,
                        rect.bottom + ring - inset,
                    );
                    canvas.outline(outer, theme.text);
                }
            }
        }
    }

    fn input(&self, input: Input, cx: &mut WidgetCx<Accent>) {
        if let Input::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = input
            && let Some(accent) = self.hit(x, y, cx.bounds())
        {
            cx.emit(accent);
        }
    }
}

/// Creates the strip as a child of `ui`, raising `on_pick` for a clicked preset.
pub(super) fn create<M: 'static>(
    ui: &mut Ui<M>,
    on_pick: impl Fn(Accent) -> Option<M> + 'static,
) -> win32ui::Result<Custom<AccentSwatches, M>> {
    Ok(Custom::new(ui, AccentSwatches::new(ui.dpi()))?.on_event(on_pick))
}
