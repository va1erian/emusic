//! Live appearance state for the Win32 frontend (#309): the [`Metrics`] and
//! zebra flag every view renders with, derived from the shared [`Appearance`]
//! and the window's DPI.
//!
//! The app installs it once whenever the font size, density, zebra flag or DPI
//! changes, then pushes it into the views. Views read the thread-local so a
//! custom widget painting mid-relayout sees the same values as the lists.

use std::cell::Cell;

use emusic_ui::state::{Appearance, Metrics};

/// The base body size in device-independent pixels, the size [`FontSize`]
/// scales. It matches egui's `TextStyle::Body` (13 logical px) so both
/// frontends derive the same relative type scale; [`points`] converts a metric
/// DIP to the point size GDI's `Font::new` expects.
///
/// [`FontSize`]: emusic_ui::state::FontSize
pub const BASE_BODY_DIP: f32 = 13.0;

/// The face the frontend draws UI text with, matching the system UI font.
pub const UI_FAMILY: &str = "Segoe UI";

/// Points per device-independent pixel at 96 DPI.
const POINTS_PER_DIP: f32 = 72.0 / 96.0;

/// Converts a layout value in device-independent pixels (what [`Metrics`]
/// carries) to the point size GDI's `Font::new` takes.
pub fn points(dip: f32) -> f32 {
    dip * POINTS_PER_DIP
}

#[derive(Clone, Copy)]
struct Runtime {
    appearance: Appearance,
    metrics: Metrics,
}

thread_local! {
    static CURRENT: Cell<Runtime> = const {
        Cell::new(Runtime {
            appearance: Appearance {
                font_size: emusic_ui::state::FontSize::Default,
                density: emusic_ui::state::Density::Comfortable,
                zebra: true,
            },
            metrics: Metrics::DEFAULT,
        })
    };
}

/// Recomputes [`Metrics`] from `appearance` at `dpi` (device DPI) and records
/// them for this thread. Returns the new metrics so the caller can apply them
/// to the views immediately.
pub fn install(appearance: Appearance, dpi: u32) -> Metrics {
    let metrics = Metrics::compute(
        appearance.font_size,
        appearance.density,
        BASE_BODY_DIP,
        dpi as f32 / 96.0,
    );
    CURRENT.with(|cell| {
        cell.set(Runtime {
            appearance,
            metrics,
        })
    });
    metrics
}

/// The metrics the views should render with.
pub fn metrics() -> Metrics {
    CURRENT.with(Cell::get).metrics
}

/// Whether list rows should alternate their background.
pub fn zebra() -> bool {
    CURRENT.with(Cell::get).appearance.zebra
}

/// The font-size multiplier (1.0 at [`FontSize::Default`](
/// emusic_ui::state::FontSize::Default)), for icons and other fixed sizes that
/// should scale with the text but are not part of [`Metrics`].
pub fn font_scale() -> f32 {
    metrics().body / BASE_BODY_DIP
}

/// Applies the current metrics and zebra flag to an owner-drawn list view:
/// the row font, the explicit (device-pixel-aligned) row height and striping.
/// This is the one place lists read their appearance from.
pub fn apply_list<T: 'static, M: 'static>(list: &win32ui::ListView<T, M>) {
    let metrics = metrics();
    list.set_row_font(UI_FAMILY, points(metrics.body));
    list.set_row_height(win32ui::Dip::new(metrics.row_height));
    list.set_zebra(zebra());
}

#[cfg(test)]
mod tests {
    use super::*;
    use emusic_ui::state::{Density, FontSize};

    #[test]
    fn default_metrics_match_the_frontend_base() {
        let metrics = install(Appearance::default(), 96);
        assert_eq!(metrics.body, BASE_BODY_DIP);
        assert_eq!(font_scale(), 1.0);
        // 13 DIP is the 9.75 pt Segoe UI body both frontends have always used.
        assert_eq!(points(metrics.body), 9.75);
    }

    #[test]
    fn installing_records_the_choices() {
        let chosen = Appearance {
            font_size: FontSize::Larger,
            density: Density::Compact,
            zebra: false,
        };
        let metrics = install(chosen, 120);
        assert!(!zebra());
        assert_eq!(metrics.body, BASE_BODY_DIP * FontSize::Larger.scale());
    }
}
