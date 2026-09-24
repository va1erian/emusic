//! Background auto-tag lookups and their backend-side state (#208).
//!
//! Looking a track up against an online database is slow network I/O, so it
//! runs on its own thread like the scanner (#69) and the tag-edit worker
//! (#172). [`AutoTagState`] owns the provider, the lookup in flight, its
//! progress line and the outcomes waiting for the UI to drain; [`spawn`] owns
//! the thread and posts the outcome back through the backend's update channel.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use emusic_metadata::Provider;

use super::Update;
use crate::library_api::{AutoTagError, AutoTagOutcome, AutoTagRequest, AutoTagStatus};

/// A shared flag a running lookup polls for cancellation.
#[derive(Clone, Default)]
pub(crate) struct AutoTagCancel(Arc<AtomicBool>);

impl AutoTagCancel {
    /// A fresh, not-yet-cancelled token.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Asks the lookup to stop as soon as it can.
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether cancellation has been requested.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// The backend's auto-tag state: the provider, the lookup in flight, its
/// progress line, and the outcomes waiting to be drained.
pub(crate) struct AutoTagState {
    provider: Arc<dyn Provider>,
    status: Option<AutoTagStatus>,
    /// The lookup in flight — its file and cancellation token — if any. At
    /// most one runs at a time (the dialog is single-track); a new request
    /// cancels the previous one.
    active: Option<(PathBuf, AutoTagCancel)>,
    results: Vec<AutoTagOutcome>,
}

impl AutoTagState {
    /// Creates the state around `provider`.
    pub(crate) fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            status: None,
            active: None,
            results: Vec::new(),
        }
    }

    /// Starts a lookup, cancelling any previous one (the dialog is
    /// single-track). `updates` receives the outcome when it finishes.
    pub(crate) fn request(&mut self, request: AutoTagRequest, updates: Sender<Update>) {
        if let Some((_, cancel)) = self.active.take() {
            cancel.cancel();
        }
        tracing::info!(path = %request.path.display(), "auto-tag lookup requested");
        let cancel = AutoTagCancel::new();
        self.status = Some(AutoTagStatus {
            path: request.path.clone(),
            text: format!("Looking up tags on {}…", self.provider.name()),
            done: 0,
            total: 1,
        });
        self.active = Some((request.path.clone(), cancel.clone()));
        spawn(self.provider.clone(), request, updates, cancel);
    }

    /// Drains the completed outcomes.
    pub(crate) fn take_results(&mut self) -> Vec<AutoTagOutcome> {
        std::mem::take(&mut self.results)
    }

    /// The progress line of the lookup in flight, if any.
    pub(crate) fn status(&self) -> Option<AutoTagStatus> {
        self.status.clone()
    }

    /// Cancels the lookup in flight, if any.
    pub(crate) fn cancel(&mut self) {
        if let Some((_, cancel)) = self.active.take() {
            cancel.cancel();
        }
        self.status = None;
    }

    /// Records a finished lookup, clearing the progress line if it was the one
    /// being tracked.
    pub(crate) fn handle_outcome(&mut self, outcome: AutoTagOutcome) {
        if self
            .status
            .as_ref()
            .is_some_and(|status| status.path == outcome.path)
        {
            self.status = None;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|(path, _)| *path == outcome.path)
        {
            self.active = None;
        }
        self.results.push(outcome);
    }

    /// Replaces the provider, so tests can drive the worker without a network.
    #[cfg(test)]
    pub(crate) fn set_provider(&mut self, provider: Arc<dyn Provider>) {
        self.provider = provider;
    }
}

/// Spawns a worker that looks `request` up through `provider` and posts the
/// outcome back through `updates`.
pub(crate) fn spawn(
    provider: Arc<dyn Provider>,
    request: AutoTagRequest,
    updates: Sender<Update>,
    cancel: AutoTagCancel,
) {
    std::thread::spawn(move || {
        let outcome = run(provider.as_ref(), &request, &cancel);
        let _ = updates.send(Update::AutoTag(outcome));
    });
}

/// Runs one lookup, reporting cancellation instead of a result if the token
/// was set before or during the provider call.
fn run(
    provider: &dyn Provider,
    request: &AutoTagRequest,
    cancel: &AutoTagCancel,
) -> AutoTagOutcome {
    let result = if cancel.is_cancelled() {
        Err(AutoTagError::Cancelled)
    } else {
        let result = provider.search(&request.query).map_err(AutoTagError::from);
        if cancel.is_cancelled() {
            Err(AutoTagError::Cancelled)
        } else {
            result
        }
    };
    AutoTagOutcome {
        path: request.path.clone(),
        result,
    }
}

#[cfg(test)]
mod tests;
