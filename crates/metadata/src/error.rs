//! Errors returned by online metadata lookups.

/// Errors a metadata [`Provider`](crate::Provider) can return.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    /// The provider could not be reached at all (no network, DNS failure, or a
    /// refused connection).
    #[error("the metadata provider could not be reached; check your connection")]
    Offline,

    /// A transport-level failure that is not clearly "offline" (timeout, TLS,
    /// protocol error). The string carries the underlying message.
    #[error("could not reach the metadata provider: {0}")]
    Network(String),

    /// The provider is rate-limiting us. MusicBrainz answers `503` when a
    /// client exceeds its one-request-per-second budget; retry after a pause.
    #[error("the metadata provider is rate-limiting requests; try again shortly")]
    RateLimited,

    /// The provider answered with an unexpected HTTP status code.
    #[error("the metadata provider returned HTTP {status}")]
    Status {
        /// The HTTP status code that was returned.
        status: u16,
    },

    /// The provider's response body could not be parsed.
    #[error("could not parse the metadata provider response: {0}")]
    Parse(String),

    /// The lookup completed but produced no candidates.
    #[error("no matching metadata was found")]
    NoMatch,
}

/// Convenience alias for results returned by this crate.
pub type Result<T> = std::result::Result<T, MetadataError>;
