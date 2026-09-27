//! The status-bar visualizer strip (#25, #277, #371), ported to the portable
//! `Canvas`: spectrum bars with peak-hold caps, or an oscilloscope trace.
//!
//! Cheap by construction: nothing is fetched or repainted unless the strip is
//! on, a mode other than Off is selected and audio is playing (the shell only
//! wakes at [`FRAME_INTERVAL`](emusic_ui::panels::visualizer::FRAME_INTERVAL)
//! then), the FFT is reduced to [`BAR_COUNT`] bars before it reaches the
//! widget, and the scope trace is decimated to [`TRACE_POINTS`]. A click cycles
//! the mode.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use emusic_ui::panels::visualizer::VisualizerState;
use emusic_ui::panels::visualizer::analysis::{
    BAR_COUNT, bars_from_fft, decay_peaks, decimate, resize_peaks,
};
use emusic_ui::player_api::{PlaybackStatus, PlayerApi};
use emusic_ui::state::VisualizerMode;
use xui::xui_core::Theme;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::{Canvas, Event, NodeKind, NodeSpec};
use xui::xui_core::geometry::{Point, Rect};
use xui::xui_core::units::{Dip, dip};
use xui::xui_core::widget::Control;

use crate::app::Msg;

/// The strip's size, in design units.
pub const WIDTH: Dip = dip(150.0);
/// The strip's height, in design units.
pub const HEIGHT: Dip = dip(22.0);
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
pub struct VisualizerStrip {
    ui: Ui<Msg>,
    control: Control<Msg>,
    frame: Rc<RefCell<Frame>>,
    /// When the strip was last fed, for the peak-hold decay interval.
    last_feed: Cell<Option<Instant>>,
}

impl VisualizerStrip {
    /// Creates the strip; a click cycles the mode.
    pub fn new(ui: &Ui<Msg>) -> VisualizerStrip {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, Rect::default()))
            .expect("create visualizer strip");
        let frame = Rc::new(RefCell::new(Frame::default()));
        {
            let frame = Rc::clone(&frame);
            let theme = ui.theme_handle();
            control.set_painter(Rc::new(move |canvas| {
                paint(canvas, &theme.get(), &frame.borrow());
            }));
        }
        control.on_events(|event| match event {
            Event::MouseUp {
                button: xui::xui_core::MouseButton::Left,
                ..
            } => Some(Msg::CycleVisualizer),
            _ => None,
        });
        VisualizerStrip {
            ui: ui.clone(),
            control,
            frame,
            last_feed: Cell::new(None),
        }
    }

    /// Updates the strip for this frame. Touches the player only while `mode`
    /// is active and something is playing; otherwise it just blanks the strip
    /// once (when it was showing something) and returns.
    pub fn feed(&self, enabled: bool, mode: VisualizerMode, player: &dyn PlayerApi) {
        let playing = player.status() == PlaybackStatus::Playing;
        let mut frame = self.frame.borrow_mut();
        if !enabled || !VisualizerState::wishes_repaint(mode, playing) {
            self.last_feed.set(None);
            if frame.mode != mode || !frame.bars.is_empty() || !frame.trace.is_empty() {
                *frame = Frame {
                    mode,
                    ..Frame::default()
                };
                drop(frame);
                self.control.invalidate();
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
        self.control.invalidate();
    }

    /// Moves/resizes the strip.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.control.id(), rect)]);
    }

    /// Shows or hides the strip.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.control.id(), visible);
    }
}

/// Draws the strip: spectrum bars with peak caps, or the oscilloscope trace.
fn paint(canvas: &mut dyn Canvas, theme: &Theme, frame: &Frame) {
    let bounds = canvas.bounds();
    canvas.fill_rect(bounds, theme.input_background);
    canvas.push_clip(bounds);
    match frame.mode {
        VisualizerMode::Oscilloscope => paint_trace(canvas, bounds, &frame.trace, theme),
        _ => paint_bars(canvas, bounds, frame, theme),
    }
    canvas.pop_clip();
}

/// Draws the spectrum bars, each with a peak-hold cap.
fn paint_bars(canvas: &mut dyn Canvas, bounds: Rect, frame: &Frame, theme: &Theme) {
    let width = bounds.width();
    let height = bounds.height();
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
fn paint_trace(canvas: &mut dyn Canvas, bounds: Rect, trace: &[f32], theme: &Theme) {
    if trace.len() < 2 {
        return;
    }
    let width = bounds.width() as f32;
    let mid = (bounds.top + bounds.bottom) as f32 / 2.0;
    let half = bounds.height() as f32 / 2.0;
    let step = width / (trace.len() - 1) as f32;
    let point = |index: usize| {
        let x = bounds.left + (index as f32 * step) as i32;
        let y = mid - trace[index].clamp(-1.0, 1.0) * half;
        Point::new(x, y as i32)
    };
    for index in 1..trace.len() {
        let (from, to) = (point(index - 1), point(index));
        canvas.draw_line(from, to, theme.accent, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_fill_keeps_a_gap_between_bars() {
        let slot = WIDTH.0 / BAR_COUNT as f32;
        let bar = slot * BAR_FILL;
        assert!(bar < slot, "the bar must leave a gap in its slot");
        assert!(bar > 0.0);
    }
}
