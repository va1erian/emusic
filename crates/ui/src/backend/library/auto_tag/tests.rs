//! Unit tests for the auto-tag worker: outcome routing and cancellation.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use emusic_metadata::{Candidate, MetadataError, Provider, TrackQuery};

use super::super::Update;
use super::{AutoTagCancel, run, spawn};
use crate::library_api::{AutoTagError, AutoTagRequest};

fn request() -> AutoTagRequest {
    AutoTagRequest {
        path: PathBuf::from("C:/music/a.flac"),
        query: TrackQuery::default(),
    }
}

/// A provider that returns a fixed candidate list.
struct StubProvider(Vec<Candidate>);

impl Provider for StubProvider {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn search(&self, _query: &TrackQuery) -> Result<Vec<Candidate>, MetadataError> {
        Ok(self.0.clone())
    }
}

/// A provider that fails the test if it is ever called.
struct PanicProvider;

impl Provider for PanicProvider {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn search(&self, _query: &TrackQuery) -> Result<Vec<Candidate>, MetadataError> {
        panic!("the provider must not be called once cancelled");
    }
}

#[test]
fn run_returns_the_provider_candidates() {
    let provider = StubProvider(vec![Candidate {
        title: Some("Title".to_string()),
        score: 0.9,
        ..Default::default()
    }]);
    let outcome = run(&provider, &request(), &AutoTagCancel::new());
    let candidates = outcome.result.expect("the stub succeeds");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title.as_deref(), Some("Title"));
}

#[test]
fn a_cancelled_lookup_never_calls_the_provider() {
    let cancel = AutoTagCancel::new();
    cancel.cancel();
    let outcome = run(&PanicProvider, &request(), &cancel);
    assert!(matches!(outcome.result, Err(AutoTagError::Cancelled)));
}

#[test]
fn spawn_posts_the_outcome_through_the_channel() {
    let provider: Arc<dyn Provider> = Arc::new(StubProvider(vec![Candidate::default()]));
    let (tx, rx) = mpsc::channel();
    spawn(provider, request(), tx, AutoTagCancel::new());

    match rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the worker posts an outcome")
    {
        Update::AutoTag(outcome) => {
            assert_eq!(outcome.path, PathBuf::from("C:/music/a.flac"));
            assert!(outcome.result.is_ok());
        }
        _ => panic!("expected an auto-tag update"),
    }
}
