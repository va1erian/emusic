//! `/admin/api/*`: the admin pages' data as JSON.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use super::pages::audit::AuditFilter;
use super::{AdminState, data, internal_error};
use crate::error::Result;

fn respond<T: Serialize>(result: Result<T>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => internal_error(&error),
    }
}

/// `GET /admin/api/overview`
pub async fn overview(State(admin): State<AdminState>) -> Response {
    respond(data::overview(&admin.app).await)
}

/// `GET /admin/api/active`
pub async fn active(State(admin): State<AdminState>) -> Response {
    respond(data::active(&admin.app).await)
}

/// `GET /admin/api/devices`
pub async fn devices(State(admin): State<AdminState>) -> Response {
    respond(data::devices(&admin.app).await)
}

/// `GET /admin/api/pairing`
pub async fn pairing(State(admin): State<AdminState>) -> Response {
    respond(data::pairing(&admin.app).await)
}

/// `GET /admin/api/audit?event=&device=&since=&until=&before=&limit=`
pub async fn audit(State(admin): State<AdminState>, Query(filter): Query<AuditFilter>) -> Response {
    match filter.to_query() {
        Ok(query) => respond(data::audit(&admin.app, query).await),
        Err(message) => (axum::http::StatusCode::BAD_REQUEST, message).into_response(),
    }
}

/// `GET /admin/api/database`
pub async fn database(State(admin): State<AdminState>) -> Response {
    respond(data::database(&admin.app).await)
}
