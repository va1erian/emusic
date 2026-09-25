//! The top-bar visualizer strip (#277): spectrum bars or an oscilloscope
//! trace, drawn with plain GDI rectangles and lines.
//!
//! Cheap by construction: nothing is fetched or repainted unless the strip is
//! on, a mode other than Off is selected and audio is playing (the shell only
//! wakes at [`FRAME_INTERVAL`](emusic_ui::panels::visualizer::FRAME_INTERVAL)
//! then), the FFT is reduced to [`BAR_COUNT`] bars before it reaches the
//! widget, and the scope trace is decimated to [`MAX_POINTS`].

use std::cell::{Cell, RefCell};
use std::time::Instant;

use emusic_ui::panels::visualizer::VisualizerState;
use emusic_ui::panels::visualizer::analysis::{
    BAR_COUNT, bars_from_fft, decay_peaks, decimate, resize_peaks,
};
use emusic_ui::player_api::{PlaybackStatus, PlayerApi};
use emusic_ui::state::VisualizerMode;
use win32ui::d2d::{PointF, Stroke};
use win32ui::gdi::Canvas;
use win32ui::prelude::*;
use win32ui::{Custom, CustomWidget, Input, MouseButton, Point, Rect, Size, Theme, WidgetCx};

use crate::app::Msg;

/// The strip's size, in design units.
pub const WIDTH: f32 = 150.0;
/// The strip's height, in design units.
pub const HEIGHT: f32 = 22.0;
/// Points in the oscilloscope trace: about one per two pixels of the strip.
const TRACE_POINTS: usize = 96;
/// Fraction of each bar's slot the bar fills (the rest is a gap).
const BAR_FILL: f32 = 0.72;

/// What the strip currently shows.
#[derive(Default)]
struct Frame {
    mode: VisualizerMode,
    bars: Vec<f32>,
    peaks: Vec<f32>,
    trace: Vec<f32>,
}

/// The owner-drawn strip.
struct VisualizerWidget {
    frame: RefCell<Frame>,
    dpi: Cell<u32>,
}

impl CustomWidget for VisualizerWidget {
    type Event = ();

    fn preferred_size(&self, dpi: u32) -> Option<Size> {
        self.dpi.set(dpi);
        Some(Size::new(
            dip(WIDTH).to_px(dpi).value(),
            dip(HEIGHT).to_px(dpi).value(),
        ))
    }

    fn paint(&self, canvas: &Canvas, bounds: Rect, theme: &Theme) {
        canvas.fill_rect(bounds, theme.input_background);
        let frame = self.frame.borrow();
        match frame.mode {
            VisualizerMode::Oscilloscope => paint_trace(canvas, bounds, &frame.trace, theme),
            _ => paint_bars(canvas, bounds, &frame, theme),
        }
    }

    fn input(&self, input: Input, cx: &mut WidgetCx<()>) {
        if matches!(
            input,
            Input::MouseDown {
                button: MouseButton::Left,
                ..
            }
        ) {
            cx.emit(());
        }
    }
}

/// Draws the spectrum bars, each with a peak-hold cap.
fn paint_bars(canvas: &Canvas, bounds: Rect, frame: &Frame, theme: &Theme) {
    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    let slot = width as f32 / BAR_COUNT as f32;
    let bar_width = ((slot * BAR_FILL) as i32).max(1);
    for (index, value) in frame.bars.iter().enumerate() {
        let left = bounds.left + (index as f32 * slot) as i32;
        let bar_height = (height as f32 * value.clamp(0.0, 1.0)) as i32;
        canvas.fill_rect(
            Rect::new(
                left,
                bounds.bottom - bar_height,
                left + bar_width,
                bounds.bottom,
            ),
            theme.accent,
        );
        let peak = frame.peaks.as_slice().get(index).copied().unwrap_or(0.0);
        if peak > 0.0 {
            let y = bounds.bottom - (height as f32 * peak.clamp(0.0, 1.0)) as i32;
            let y = y.max(bounds.top + 1);
            canvas.fill_rect(Rect::new(left, y - 1, left + bar_width, y), theme.text);
        }
    }
}

/// Draws the decimated samples as one polyline centred vertically.
fn paint_trace(canvas: &Canvas, bounds: Rect, trace: &[f32], theme: &Theme) {
    if trace.len() < 2 {
        return;
    }
    let width = (bounds.right - bounds.left) as f32;
    let mid = (bounds.top + bounds.bottom) as f32 / 2.0;
    let half = (bounds.bottom - bounds.top) as f32 / 2.0;
    let step = width / (trace.len() - 1) as f32;
    let point = |index: usize| {
        let x = bounds.left + (index as f32 * step) as i32;
        let y = mid - trace[index].clamp(-1.0, 1.0) * half;
        Point::new(x, y as i32)
    };
    // One Direct2D canvas for the whole trace: `Canvas::line` builds a fresh
    // render target per call, which made a 256-segment scope crawl.
    let Some(mut d2d) = canvas.d2d() else {
        return;
    };
    for index in 1..trace.len() {
        let (from, to) = (point(index - 1), point(index));
        d2d.draw_line(
            PointF::new(from.x as f32 + 0.5, from.y as f32 + 0.5),
            PointF::new(to.x as f32 + 0.5, to.y as f32 + 0.5),
            theme.accent,
            Stroke::solid(1.0),
        );
    }
    let _ = d2d.end_draw();
}

/// The strip and its per-frame feed.
pub struct VisualizerView {
    custom: Custom<VisualizerWidget, Msg>,
    last_feed: Cell<Option<Instant>>,
}

impl VisualizerView {
    /// Creates the strip; a click cycles the mode.
    pub fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let widget = VisualizerWidget {
            frame: RefCell::new(Frame::default()),
            dpi: Cell::new(ui.dpi()),
        };
        let custom = Custom::new(ui, widget)?.on_event(|()| Some(Msg::CycleVisualizer));
        Ok(Self {
            custom,
            last_feed: Cell::new(None),
        })
    }

    /// Updates the strip for this frame. Touches the player only while `mode`
    /// is active and something is playing; otherwise it just blanks the strip
    /// once (when it was showing something) and returns.
    pub fn feed(&self, mode: VisualizerMode, player: &dyn PlayerApi) {
        let playing = player.status() == PlaybackStatus::Playing;
        let widget = self.custom.widget();
        let widget = widget.borrow();
        let mut frame = widget.frame.borrow_mut();

        if !VisualizerState::wishes_repaint(mode, playing) {
            self.last_feed.set(None);
            if frame.mode != mode || !frame.bars.is_empty() || !frame.trace.is_empty() {
                *frame = Frame {
                    mode,
                    ..Frame::default()
                };
                drop(frame);
                self.custom.invalidate();
            }
            return;
        }

        let now = Instant::now();
        let dt = self
            .last_feed
            .replace(Some(now))
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        frame.mode = mode;
        if mode == VisualizerMode::Oscilloscope {
            frame.trace = decimate(&player.samples(), TRACE_POINTS);
        } else {
            frame.bars = bars_from_fft(&player.fft(), BAR_COUNT);
            resize_peaks(&mut frame.peaks, BAR_COUNT);
            decay_peaks(&mut frame.peaks, dt);
            let Frame { bars, peaks, .. } = &mut *frame;
            for (peak, bar) in peaks.iter_mut().zip(bars.iter()) {
                *peak = peak.max(*bar);
            }
        }
        drop(frame);
        self.custom.invalidate();
    }
}

impl AsControl for VisualizerView {
    fn control(&self) -> &Control {
        self.custom.control()
    }
}
