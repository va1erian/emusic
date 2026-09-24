//! The [`Provider`] trait every metadata database implements.

use crate::{Candidate, MetadataError, Result, TrackQuery};

/// An online metadata database that can rank candidates for a track.
///
/// Implementations must be `Send + Sync` so a backend can drive one from a
/// worker thread. [`Provider::search`] is blocking and is always called off the
/// UI thread.
pub trait Provider: Send + Sync {
    /// A short, human-readable provider name, for the UI.
    fn name(&self) -> &'static str;

    /// Returns candidates for `query`, best first.
    ///
    /// An empty result is not an error; use [`Provider::best_match`] when at
    /// least one match is required.
    fn search(&self, query: &TrackQuery) -> Result<Vec<Candidate>>;

    /// Returns the highest-scoring candidate, or [`MetadataError::NoMatch`]
    /// when the provider found nothing.
    fn best_match(&self, query: &TrackQuery) -> Result<Candidate> {
        let mut candidates = self.search(query)?;
        if candidates.is_empty() {
            return Err(MetadataError::NoMatch);
        }
        candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
        candidates.into_iter().next().ok_or(MetadataError::NoMatch)
    }
}
