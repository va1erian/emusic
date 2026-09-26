//! Request extractors: peer/client address and bearer-token authentication.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use crate::api::error::ApiError;
use crate::auth::paseto::verify_access_token;
use crate::db::models::Device;
use crate::security::client_ip;
use crate::state::AppState;
use crate::util::unix_now;

/// The direct peer address, or loopback when unavailable (e.g. tests).
#[derive(Debug, Clone, Copy)]
pub struct PeerAddr(pub IpAddr);

impl<S> FromRequestParts<S> for PeerAddr
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(PeerAddr(peer_ip(parts)))
    }
}

/// The real client address, honouring trusted proxy headers.
#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub IpAddr);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = peer_ip(parts);
        let forwarded = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok());
        Ok(ClientIp(client_ip(peer, &state.trusted, forwarded)))
    }
}

/// An authenticated device extracted from a valid bearer token.
#[derive(Debug, Clone)]
pub struct AuthDevice(pub Device);

impl AuthDevice {
    /// The authenticated device.
    pub fn device(&self) -> &Device {
        &self.0
    }

    /// The authenticated device's id.
    pub fn id(&self) -> &str {
        &self.0.id
    }
}

impl FromRequestParts<AppState> for AuthDevice {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts).ok_or_else(|| {
            crate::audit::auth_failed(&client_string(parts, state), "missing token");
            ApiError::unauthorized("missing bearer token")
        })?;

        let verified = verify_access_token(&state.keys, token).map_err(|error| {
            crate::audit::auth_failed(&client_string(parts, state), "invalid token");
            ApiError::from(error)
        })?;

        let db = state.db.clone();
        let device_id = verified.device_id.clone();
        let device = tokio::task::spawn_blocking(move || db.device_by_id(&device_id))
            .await
            .map_err(|_| ApiError::internal())?
            .map_err(ApiError::from)?;

        let Some(device) = device else {
            crate::audit::auth_failed(&client_string(parts, state), "unknown device");
            return Err(ApiError::unauthorized("unknown device"));
        };
        if device.is_revoked {
            crate::audit::auth_failed(&client_string(parts, state), "revoked device");
            return Err(ApiError::unauthorized("device revoked"));
        }

        let db = state.db.clone();
        let id = device.id.clone();
        let _ = tokio::task::spawn_blocking(move || db.touch_device(&id, unix_now())).await;

        Ok(AuthDevice(device))
    }
}

/// Bearer token from the `Authorization` header, if well-formed.
pub fn bearer_token(parts: &Parts) -> Option<&str> {
    let value = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

fn peer_ip(parts: &Parts) -> IpAddr {
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn client_string(parts: &Parts, state: &AppState) -> String {
    let forwarded = parts
        .headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok());
    client_ip(peer_ip(parts), &state.trusted, forwarded).to_string()
}
