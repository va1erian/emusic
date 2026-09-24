//! Click hit-testing and input handling for the now-playing summary (#110).
//!
//! The clickable regions are recorded by [`super::draw`] on each paint; input
//! looks them up and raises the matching [`SummaryEvent`].

use win32ui::prelude::*;

use super::{SummaryEvent, SummaryWidget};

/// A clickable region of the summary, as laid out by the last paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Hit {
    Star,
    Artist,
    Album,
    Path,
    Properties,
    EditTags,
}

impl Hit {
    fn event(self) -> SummaryEvent {
        match self {
            Hit::Star => SummaryEvent::ToggleStar,
            Hit::Artist => SummaryEvent::GoToArtist,
            Hit::Album => SummaryEvent::GoToAlbum,
            Hit::Path => SummaryEvent::OpenFolder,
            Hit::Properties => SummaryEvent::ShowProperties,
            Hit::EditTags => SummaryEvent::EditTags,
        }
    }
}

/// The clickable region under a client point, if any.
fn hit(widget: &SummaryWidget, x: i32, y: i32) -> Option<Hit> {
    let point = Point::new(x, y);
    widget
        .hits
        .borrow()
        .iter()
        .find(|(rect, _)| rect.contains(point))
        .map(|(_, hit)| *hit)
}

/// Handles one input event, emitting a [`SummaryEvent`] on a completed click.
pub(super) fn handle(widget: &SummaryWidget, input: Input, cx: &mut WidgetCx<SummaryEvent>) {
    match input {
        Input::MouseMove { x, y, .. } => {
            let hit = hit(widget, x, y);
            if hit != widget.hot.get() {
                widget.hot.set(hit);
                cx.invalidate();
            }
        }
        Input::MouseLeave => {
            if widget.hot.take().is_some() {
                cx.invalidate();
            }
        }
        Input::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => widget.pressed.set(hit(widget, x, y)),
        Input::MouseUp {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => {
            let hit = hit(widget, x, y);
            if hit.is_some()
                && hit == widget.pressed.take()
                && let Some(hit) = hit
            {
                cx.emit(hit.event());
            }
        }
        _ => {}
    }
}
