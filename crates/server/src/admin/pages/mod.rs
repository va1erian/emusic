//! Server-rendered admin pages and their form actions.

pub mod active;
pub mod audit;
pub mod database;
pub mod devices;
pub mod overview;
pub mod pairing;

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;

use super::{AdminState, internal_error};
use crate::error::Result;
use crate::util::unix_now;

/// A POST body carrying only the CSRF token.
#[derive(Debug, Deserialize)]
pub struct CsrfForm {
    /// The form token.
    pub csrf: String,
}

/// The admin client's address (the TCP peer; no proxy is trusted here).
pub struct PeerIp(pub String);

impl FromRequestParts<AdminState> for PeerIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AdminState,
    ) -> std::result::Result<Self, Self::Rejection> {
        let ip = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip().to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        Ok(Self(ip))
    }
}

/// Renders a page, or a detail-free `500`.
fn render(result: Result<Html<String>>) -> Response {
    match result {
        Ok(html) => html.into_response(),
        Err(error) => internal_error(&error),
    }
}

/// Refuses a form post that is cross-origin or carries a bad token.
fn reject_forged(admin: &AdminState, headers: &HeaderMap, token: &str) -> Option<Response> {
    if super::csrf::is_same_origin(headers) && admin.csrf.verify(token, unix_now()) {
        return None;
    }
    Some(
        (
            StatusCode::FORBIDDEN,
            "invalid or expired form token; reload the page",
        )
            .into_response(),
    )
}
