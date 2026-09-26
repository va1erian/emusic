//! Auth verification middleware.

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

use crate::AppState;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

    if !auth_header.starts_with("Bearer ") {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization scheme"));
    }

    let token = &auth_header["Bearer ".len()..];
    let device_id = state
        .token_manager
        .verify_token(token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid PASETO token"))?;

    let device = state
        .db
        .get_device(&device_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "Device not registered"))?;

    if device.is_revoked {
        return Err((StatusCode::UNAUTHORIZED, "Device has been revoked"));
    }

    let _ = state.db.touch_device(&device_id);

    req.extensions_mut().insert(device_id);
    Ok(next.run(req).await)
}
