//! The now-playing panel's summary/queue split (#514): how tall the play queue
//! list under the summary is, and the bounds a drag of the boundary between the
//! two is held to.
//!
//! Heights are in device-independent pixels. The frontend reports a drag as a
//! delta plus the height the summary and the queue currently share; this module
//! turns that into the new queue height, so the clamping is toolkit-agnostic
//! and unit-tested.

/// The queue list's default height, in DIP: the panel's layout before it could
/// be resized. Double-clicking the splitter returns to it.
pub const DEFAULT_QUEUE_HEIGHT: f32 = 200.0;
/// The smallest queue height, in DIP: the column header plus about three rows.
pub const MIN_QUEUE_HEIGHT: f32 = 96.0;
/// The smallest summary height a drag leaves above the queue, in DIP: the
/// title and the artist/album lines without the artwork (the summary scrolls
/// to show the rest).
pub const MIN_SUMMARY_HEIGHT: f32 = 72.0;
/// The largest queue height accepted from the config, in DIP. The real upper
/// bound depends on the panel's height, which [`drag_queue_height`] knows; this
/// only rejects absurd hand-edited values before the first layout.
pub const MAX_QUEUE_HEIGHT: f32 = 4000.0;

/// A persisted queue height made safe to lay out: non-finite values fall back
/// to [`DEFAULT_QUEUE_HEIGHT`], others are held to
/// [`MIN_QUEUE_HEIGHT`]`..=`[`MAX_QUEUE_HEIGHT`].
#[must_use]
pub fn sanitize_queue_height(height: f32) -> f32 {
    if height.is_finite() {
        height.clamp(MIN_QUEUE_HEIGHT, MAX_QUEUE_HEIGHT)
    } else {
        DEFAULT_QUEUE_HEIGHT
    }
}

/// Clamps `height` so the queue keeps [`MIN_QUEUE_HEIGHT`] and the summary
/// above it keeps [`MIN_SUMMARY_HEIGHT`] of the `span` the two share (the
/// splitter excluded). When the span is too short for both minimums the queue
/// keeps its own.
#[must_use]
pub fn clamp_queue_height(height: f32, span: f32) -> f32 {
    let max = if span.is_finite() {
        (span - MIN_SUMMARY_HEIGHT).clamp(MIN_QUEUE_HEIGHT, MAX_QUEUE_HEIGHT)
    } else {
        MAX_QUEUE_HEIGHT
    };
    sanitize_queue_height(height).min(max)
}

/// The queue height after dragging the splitter by `delta` DIP (positive is
/// downwards, which shrinks the queue) from `current`, clamped by
/// [`clamp_queue_height`] against the shared `span`.
#[must_use]
pub fn drag_queue_height(current: f32, delta: f32, span: f32) -> f32 {
    clamp_queue_height(current - delta, span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_sane_values_and_rejects_garbage() {
        assert_eq!(sanitize_queue_height(DEFAULT_QUEUE_HEIGHT), 200.0);
        assert_eq!(sanitize_queue_height(10.0), MIN_QUEUE_HEIGHT);
        assert_eq!(sanitize_queue_height(-50.0), MIN_QUEUE_HEIGHT);
        assert_eq!(sanitize_queue_height(1.0e9), MAX_QUEUE_HEIGHT);
        assert_eq!(sanitize_queue_height(f32::NAN), DEFAULT_QUEUE_HEIGHT);
        assert_eq!(sanitize_queue_height(f32::INFINITY), DEFAULT_QUEUE_HEIGHT);
    }

    #[test]
    fn dragging_up_grows_the_queue_and_down_shrinks_it() {
        assert_eq!(drag_queue_height(200.0, -50.0, 600.0), 250.0);
        assert_eq!(drag_queue_height(200.0, 30.0, 600.0), 170.0);
    }

    #[test]
    fn the_summary_keeps_its_minimum_above_the_queue() {
        let span = 600.0;
        assert_eq!(
            drag_queue_height(200.0, -1000.0, span),
            span - MIN_SUMMARY_HEIGHT
        );
    }

    #[test]
    fn the_queue_keeps_about_three_rows() {
        assert_eq!(drag_queue_height(200.0, 1000.0, 600.0), MIN_QUEUE_HEIGHT);
    }

    #[test]
    fn a_span_too_short_for_both_minimums_favours_the_queue() {
        assert_eq!(clamp_queue_height(300.0, 120.0), MIN_QUEUE_HEIGHT);
        assert_eq!(clamp_queue_height(300.0, f32::NAN), 300.0);
    }
}
