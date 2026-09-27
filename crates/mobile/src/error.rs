//! The error crossing the uniffi boundary.
//!
//! Every failure is flattened to a single message so Kotlin only has to catch
//! one exception type; the phrasing comes from [`emusic_client::ClientError`].

use emusic_client::ClientError;

/// Errors surfaced to the Android layer.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MobileError {
    /// Any client, token, network or local-storage failure.
    #[error("{message}")]
    Client {
        /// Human-readable description.
        message: String,
    },
}

impl From<ClientError> for MobileError {
    fn from(error: ClientError) -> Self {
        Self::Client {
            message: error.to_string(),
        }
    }
}

impl From<std::io::Error> for MobileError {
    fn from(error: std::io::Error) -> Self {
        Self::Client {
            message: error.to_string(),
        }
    }
}
