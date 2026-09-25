//! Live appearance state for the egui frontend (#309): the [`Metrics`] and
//! zebra flag the views render with, derived from the shared
//! [`Appearance`] and the context's current DPI.
//!
//! Installed once whenever the theme is applied (see `theme::apply`); views
//! read it with [`metrics`]/[`zebra`] instead of hard-coding row heights or
//! striping. Kept in a thread-local because egui renders on a single thread,
//! and a per-thread value also keeps parallel snapshot tests isolated.

use std::cell::Cell;

use emusic_ui::state::{Appearance, Metrics};

/// egui's default body text style size, the base that [`FontSize`] scales.
///
/// [`FontSize`]: emusic_ui::state::FontSize
const BASE_BODY_POINTS: f32 = 13.0;

#[derive(Clone, Copy)]
struct Runtime {
    metrics: Metrics,
    zebra: bool,
}

thread_local! {
    static CURRENT: Cell<Runtime> = const {
        Cell::new(Runtime {
            metrics: Metrics::DEFAULT,
            zebra: true,
        })
    };
}

/// Recomputes [`Metrics`] from the appearance choices at `dpi` and records
/// them (plus the zebra flag) for this thread. Returns the new metrics so the
/// caller (theme application) can use them immediately.
pub fn install(appearance: Appearance, dpi: f32) -> Metrics {
    let metrics = Metrics::compute(
        appearance.font_size,
        appearance.density,
        BASE_BODY_POINTS,
        dpi,
    );
    CURRENT.with(|cell| {
        cell.set(Runtime {
            metrics,
            zebra: appearance.zebra,
        })
    });
    metrics
}

/// The metrics the views should render with this frame.
pub fn metrics() -> Metrics {
    CURRENT.with(Cell::get).metrics
}

/// Whether list rows should alternate their background.
pub fn zebra() -> bool {
    CURRENT.with(Cell::get).zebra
}
