//! Frontend-agnostic waker (#95).
//!
//! Background workers (the IPC listener, thumbnail decodes, library scans,
//! player events, ...) run off the UI thread and need to ask the frontend to
//! repaint when they have something new. Hard-coding an `egui::Context` for
//! that would tie every worker to one toolkit, so they instead hold a
//! [`WakerHandle`] and the frontend binds its own [`Waker`] once the UI exists.
//!
//! The binding is late because some workers start before the window does (the
//! single-instance listener is acquired before `eframe::run_native` creates
//! the egui context); until then a handle's [`Waker::wake`] is a harmless
//! no-op. This is what the egui-bound `RepaintHandle` used to do, generalised
//! so a second frontend can implement it too.

use std::sync::{Arc, OnceLock};

/// Something that can ask a frontend to repaint its UI.
///
/// Implemented by each frontend (egui: `ctx.request_repaint()`). The wake is
/// only a hint to redraw; callers must not assume it runs synchronously.
pub trait Waker: Send + Sync + 'static {
    /// Requests that the UI repaint at its next opportunity.
    fn wake(&self);
}

/// A late-bindable [`Waker`].
///
/// Created before the UI exists, then bound once with the frontend's real
/// waker; handing out [`Self::handle`]s to workers works either way. The
/// first [`Self::bind`] wins, so a second frontend taking over (or a
/// redundant bind) is ignored rather than panicking.
#[derive(Clone, Default)]
pub struct WakerSlot {
    inner: Arc<OnceLock<Box<dyn Waker>>>,
}

impl WakerSlot {
    /// Creates an unbound slot (its handles are no-ops until [`Self::bind`]).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Connects the slot to the frontend's real waker. Later calls are
    /// ignored.
    pub fn bind(&self, waker: impl Waker) {
        let _ = self.inner.set(Box::new(waker));
    }

    /// A cloneable handle for background workers, valid before or after
    /// [`Self::bind`].
    #[must_use]
    pub fn handle(&self) -> WakerHandle {
        WakerHandle(Arc::clone(&self.inner))
    }
}

impl Waker for WakerSlot {
    fn wake(&self) {
        if let Some(waker) = self.inner.get() {
            waker.wake();
        }
    }
}

/// A shared handle to a [`WakerSlot`]'s waker.
///
/// Cheap to clone and safe to move into worker threads; a no-op until the
/// slot is bound. An unbound handle (e.g. from [`Default`]) is equally
/// harmless, which lets worker owners construct without a slot.
#[derive(Clone, Default)]
pub struct WakerHandle(Arc<OnceLock<Box<dyn Waker>>>);

impl Waker for WakerHandle {
    fn wake(&self) {
        if let Some(waker) = self.0.get() {
            waker.wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A [`Waker`] that counts how often it was woken.
    #[derive(Default)]
    struct Counter(AtomicUsize);

    impl Waker for Arc<Counter> {
        fn wake(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn unbound_handle_wake_is_a_noop() {
        let slot = WakerSlot::new();
        let handle = slot.handle();
        handle.wake();
        slot.wake();
    }

    #[test]
    fn handle_wakes_once_bound() {
        let slot = WakerSlot::new();
        let handle = slot.handle();
        let counter = Arc::new(Counter::default());

        slot.bind(Arc::clone(&counter));
        handle.wake();
        handle.wake();

        assert_eq!(counter.0.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn handles_share_the_bound_waker() {
        let slot = WakerSlot::new();
        let first = slot.handle();
        let second = slot.handle();
        let counter = Arc::new(Counter::default());

        slot.bind(Arc::clone(&counter));
        first.wake();
        second.wake();

        assert_eq!(counter.0.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn first_bind_wins() {
        let slot = WakerSlot::new();
        let first = Arc::new(Counter::default());
        let second = Arc::new(Counter::default());

        slot.bind(Arc::clone(&first));
        slot.bind(Arc::clone(&second));
        slot.handle().wake();

        assert_eq!(first.0.load(Ordering::SeqCst), 1);
        assert_eq!(second.0.load(Ordering::SeqCst), 0);
    }
}
