#![forbid(unsafe_code)]

//! The projectM stop grace period (#305).
//!
//! Hiding the visualization stops its ticks immediately, but recreating the
//! projectM instance compiles shaders and loads a preset on the UI thread.
//! [`GraceTimer`] keeps a stopped instance alive for [`GRACE`] so a quick
//! toggle, a minimise/restore or a placement change resumes without rebuilding;
//! once the deadline passes the caller frees the instance through the GL hook.

use std::time::{Duration, Instant};

/// How long a stopped visualization keeps its instance before freeing it.
pub(crate) const GRACE: Duration = Duration::from_secs(10);

/// A one-shot deadline from which the caller can schedule and poll.
#[derive(Debug, Default)]
pub(crate) struct GraceTimer {
    deadline: Option<Instant>,
}

impl GraceTimer {
    /// Arms the grace period from `now`, unless it is already armed.
    pub(crate) fn arm(&mut self, now: Instant) {
        if self.deadline.is_none() {
            self.deadline = Some(now + GRACE);
        }
    }

    /// Disarms it: the visualization started again, so nothing is pending.
    pub(crate) fn disarm(&mut self) {
        self.deadline = None;
    }

    /// How long until the instance should be freed, so the caller can schedule
    /// a wake for it. `None` when nothing is pending.
    pub(crate) fn wait(&self, now: Instant) -> Option<Duration> {
        self.deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    /// Whether the deadline has passed, clearing it when so.
    pub(crate) fn take_due(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.deadline = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_armed_timer_waits_the_grace_period_then_is_due() {
        let start = Instant::now();
        let mut timer = GraceTimer::default();
        timer.arm(start);
        assert_eq!(timer.wait(start), Some(GRACE));
        let almost = start + GRACE - Duration::from_millis(1);
        assert!(!timer.take_due(almost));
        assert_eq!(timer.wait(almost), Some(Duration::from_millis(1)));
        assert!(timer.take_due(start + GRACE));
        assert_eq!(timer.wait(start + GRACE), None, "a due timer clears itself");
    }

    #[test]
    fn rearming_does_not_extend_the_deadline() {
        let start = Instant::now();
        let mut timer = GraceTimer::default();
        timer.arm(start);
        timer.arm(start + Duration::from_secs(9));
        assert_eq!(timer.wait(start), Some(GRACE));
        assert!(timer.take_due(start + GRACE));
    }

    #[test]
    fn disarming_leaves_nothing_pending() {
        let start = Instant::now();
        let mut timer = GraceTimer::default();
        timer.arm(start);
        timer.disarm();
        assert_eq!(timer.wait(start), None);
        assert!(!timer.take_due(start + GRACE));
    }

    #[test]
    fn an_idle_timer_is_never_due() {
        let mut timer = GraceTimer::default();
        assert_eq!(timer.wait(Instant::now()), None);
        assert!(!timer.take_due(Instant::now()));
    }
}
