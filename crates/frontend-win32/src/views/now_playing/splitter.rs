//! The drag handle between the now-playing panel's summary and its queue list
//! (#514).
//!
//! It looks and behaves like the shell's `win32ui` split dividers (#342): a
//! thin band in the window background with a one-pixel border line, drawn in
//! the accent while hovered or dragged, and a vertical resize cursor. Unlike
//! those it can be double-clicked to reset the queue to its default height,
//! so it is a [`CustomWidget`] of its own. It only reports gestures; the panel
//! turns a drag into a clamped height (see
//! [`emusic_ui::state::drag_queue_height`]) and the app relayouts.

use std::cell::Cell;

use win32ui::accessibility::{AccessCx, Node, Role};
use win32ui::gdi::Canvas;
use win32ui::prelude::*;
use win32ui::{CursorShape, CustomWidget, Input, MouseButton, Rect, Size, Theme, WidgetCx};
use win32ui::{Dip, Px};

/// The handle's thickness, in DIP: the same as the shell's split dividers.
pub(super) const SPLITTER_HEIGHT: f32 = 5.0;

/// A gesture on the queue splitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitterEvent {
    /// The handle was dragged by this many DIP since the last event (positive
    /// is downwards, shrinking the queue).
    Dragged(Dip),
    /// The handle was double-clicked: back to the default queue height.
    Reset,
}

/// The owner-drawn queue splitter.
pub(super) struct QueueSplitter {
    hovered: Cell<bool>,
    dragging: Cell<bool>,
    /// Where the drag grabbed the handle, in its client pixels. The handle
    /// follows the drag through each relayout, so the cursor's offset from it
    /// is the next step.
    grab: Cell<i32>,
    /// The queue height it currently separates, in DIP, exposed as the UI
    /// Automation value so tests can read it.
    queue_height: Cell<f32>,
}

impl QueueSplitter {
    pub(super) fn new(queue_height: f32) -> Self {
        Self {
            hovered: Cell::new(false),
            dragging: Cell::new(false),
            grab: Cell::new(0),
            queue_height: Cell::new(queue_height),
        }
    }

    /// Records the queue height shown as the accessibility value.
    pub(super) fn set_queue_height(&self, height: f32) {
        self.queue_height.set(height);
    }

    fn end_drag(&self, cx: &WidgetCx<SplitterEvent>) {
        self.dragging.set(false);
        cx.invalidate();
    }
}

impl CustomWidget for QueueSplitter {
    type Event = SplitterEvent;

    fn paint(&self, canvas: &Canvas, bounds: Rect, theme: &Theme) {
        canvas.fill_rect(bounds, theme.background);
        let active = self.dragging.get() || self.hovered.get();
        let (color, half) = if active {
            (theme.accent, 2)
        } else {
            (theme.border, 1)
        };
        let y = bounds.height() / 2;
        canvas.fill_rect(
            Rect::new(bounds.left, y - half, bounds.right, y + half),
            color,
        );
    }

    fn input(&self, input: Input, cx: &mut WidgetCx<SplitterEvent>) {
        match input {
            Input::MouseMove { y, .. } => {
                if !self.hovered.replace(true) {
                    cx.invalidate();
                }
                cx.cursor(CursorShape::SizeVertical);
                let step = y - self.grab.get();
                if self.dragging.get() && step != 0 {
                    cx.emit(SplitterEvent::Dragged(Px(step).to_dip(cx.dpi())));
                }
            }
            Input::MouseDown {
                y,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging.set(true);
                self.grab.set(y);
                cx.capture();
                cx.invalidate();
            }
            Input::MouseUp {
                button: MouseButton::Left,
                ..
            } if self.dragging.get() => {
                cx.release_capture();
                self.end_drag(cx);
            }
            Input::MouseDoubleClick {
                button: MouseButton::Left,
                ..
            } => cx.emit(SplitterEvent::Reset),
            Input::CaptureChanged if self.dragging.get() => self.end_drag(cx),
            Input::MouseLeave => {
                let was_hovered = self.hovered.replace(false);
                if was_hovered {
                    cx.invalidate();
                }
            }
            _ => {}
        }
    }

    fn accessibility(&self, cx: &AccessCx) -> Option<Node> {
        Some(
            Node::new(Role::Separator, "Queue splitter")
                .id("queue-splitter")
                .value(format!("{:.0}", self.queue_height.get()))
                .help("Drag to resize the play queue; double-click to reset it")
                .bounds(cx.bounds()),
        )
    }

    fn preferred_size(&self, dpi: u32) -> Option<Size> {
        let thickness = dip(SPLITTER_HEIGHT).to_px(dpi).value();
        Some(Size::new(thickness, thickness))
    }
}
