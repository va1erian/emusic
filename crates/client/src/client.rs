//! The blocking HTTP client for the server REST API.
//!
//! One [`RemoteClient`] per server; it owns a `ureq` agent with a bounded
//! timeout so a stalled server cannot hang a worker thread. Non-2xx responses
//! are parsed for the server's `{"error": ...}` body.

use std::io::Write;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::config::{ServerEndpoint, normalize_url};
use crate::error::{ClientError, Result};
use crate::types::{ApiErrorBody, Health, PairResponse, SyncDelta, TokenResponse, TrackView};

/// A client bound to one server base URL.
#[derive(Debug, Clone)]
pub struct RemoteClient {
    base: String,
    agent: ureq::Agent,
}

impl RemoteClient {
    /// Builds a client for a base URL such as `https://music.example.com`.
    pub fn new(base: &str) -> Result<Self> {
        let base = normalize_url(base)?;
        let config = ureq::Agent::config_builder()
            // Read the status ourselves so we can surface the JSON error body.
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build();
        Ok(Self {
            base,
            agent: ureq::Agent::new_with_config(config),
        })
    }

    /// Builds a client from a configured endpoint.
    pub fn from_endpoint(endpoint: &ServerEndpoint) -> Result<Self> {
        Self::new(&endpoint.url)
    }

    /// The normalized base URL.
    pub fn base_url(&self) -> &str {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn bearer<B>(request: ureq::RequestBuilder<B>, token: &str) -> ureq::RequestBuilder<B> {
        request.header("Authorization", format!("Bearer {token}"))
    }

    fn decode<T: DeserializeOwned>(response: http::Response<ureq::Body>) -> Result<T> {
        let status = response.status().as_u16();
        if status == 401 {
            return Err(ClientError::Unauthorized);
        }
        if !(200..300).contains(&status) {
            return Err(ClientError::Http {
                status,
                message: error_message(response),
            });
        }
        response
            .into_body()
            .read_json::<T>()
            .map_err(|error| ClientError::Protocol(error.to_string()))
    }

    fn decode_text(response: http::Response<ureq::Body>) -> Result<String> {
        let status = response.status().as_u16();
        if status == 401 {
            return Err(ClientError::Unauthorized);
        }
        if !(200..300).contains(&status) {
            return Err(ClientError::Http {
                status,
                message: error_message(response),
            });
        }
        response
            .into_body()
            .read_to_string()
            .map_err(|error| ClientError::Protocol(error.to_string()))
    }

    /// `GET /api/v1/health`.
    pub fn health(&self) -> Result<Health> {
        let response = self
            .agent
            .get(self.url("/api/v1/health"))
            .call()
            .map_err(map_transport)?;
        Self::decode(response)
    }

    /// `POST /api/v1/auth/pair`.
    pub fn pair(
        &self,
        pairing_code: &str,
        device_name: &str,
        public_key: &str,
    ) -> Result<PairResponse> {
        let body = json!({
            "pairing_code": pairing_code,
            "device_name": device_name,
            "public_key": public_key,
        });
        let response = self
            .agent
            .post(self.url("/api/v1/auth/pair"))
            .send_json(&body)
            .map_err(map_transport)?;
        Self::decode(response)
    }

    /// `POST /api/v1/auth/refresh`.
    pub fn refresh(&self, token: &str, proof: &str) -> Result<TokenResponse> {
        let body = json!({ "proof": proof });
        let response = Self::bearer(self.agent.post(self.url("/api/v1/auth/refresh")), token)
            .send_json(&body)
            .map_err(map_transport)?;
        Self::decode(response)
    }

    /// `GET /api/v1/library/sync`.
    pub fn sync(&self, token: &str, since_version: i64) -> Result<SyncDelta> {
        let url = self.url(&format!(
            "/api/v1/library/sync?since_version={since_version}"
        ));
        let response = Self::bearer(self.agent.get(url), token)
            .call()
            .map_err(map_transport)?;
        Self::decode(response)
    }

    /// `GET /api/v1/tracks/{id}/meta`.
    pub fn track_meta(&self, token: &str, track_id: &str) -> Result<TrackView> {
        let url = self.url(&format!("/api/v1/tracks/{track_id}/meta"));
        let response = Self::bearer(self.agent.get(url), token)
            .call()
            .map_err(map_transport)?;
        Self::decode(response)
    }

    /// `GET /api/v1/sid/songlengths` (HVSC text).
    pub fn songlengths(&self, token: &str) -> Result<String> {
        let response = Self::bearer(self.agent.get(self.url("/api/v1/sid/songlengths")), token)
            .call()
            .map_err(map_transport)?;
        Self::decode_text(response)
    }

    /// Streams a track's bytes into `writer`, returning the byte count.
    pub fn download_to(&self, token: &str, track_id: &str, writer: &mut impl Write) -> Result<u64> {
        let url = self.url(&format!("/api/v1/tracks/{track_id}/stream"));
        let response = Self::bearer(self.agent.get(url), token)
            .call()
            .map_err(map_transport)?;
        let status = response.status().as_u16();
        if status == 401 {
            return Err(ClientError::Unauthorized);
        }
        if !(200..300).contains(&status) {
            return Err(ClientError::Http {
                status,
                message: error_message(response),
            });
        }
        let mut reader = response.into_body().into_reader();
        let copied = std::io::copy(&mut reader, writer)?;
        Ok(copied)
    }
}

/// Extracts the server's error message, falling back to the status text.
fn error_message(mut response: http::Response<ureq::Body>) -> String {
    let status = response.status();
    match response.body_mut().read_json::<ApiErrorBody>() {
        Ok(body) => body.error,
        Err(_) => format!("HTTP {status}"),
    }
}

/// Maps a transport-level `ureq` error.
fn map_transport(error: ureq::Error) -> ClientError {
    match error {
        ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => {
            ClientError::Network("cannot reach the server".into())
        }
        other => ClientError::Network(other.to_string()),
    }
}
