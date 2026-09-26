//! Auth and pairing API routes.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;
use crate::auth::pairing::generate_pairing_code;

#[derive(Debug, Deserialize)]
pub struct PairRequest {
    pub pairing_code: String,
    pub device_name: String,
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct PairResponse {
    pub device_id: String,
    pub auth_token: String,
}

pub async fn pair(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PairRequest>,
) -> Result<Json<PairResponse>, (StatusCode, String)> {
    let client_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    if !state.rate_limiter.check_and_record(
        client_ip,
        state.config.security.max_pairing_attempts_per_min,
        std::time::Duration::from_secs(60),
    ) {
        return Err((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded for pairing attempts".to_string()));
    }

    let valid = state
        .db
        .verify_and_consume_pairing_code(&payload.pairing_code)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Invalid or expired pairing code".to_string(),
        ));
    }

    let device_id = format!("dev-{}", generate_pairing_code());
    state
        .db
        .register_device(&device_id, &payload.device_name, &payload.public_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = state
        .token_manager
        .issue_token(&device_id, state.config.security.token_ttl_hours)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PairResponse {
        device_id,
        auth_token: token,
    }))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    axum::Extension(device_id): axum::Extension<String>,
) -> Result<Json<PairResponse>, (StatusCode, String)> {
    let dev = state
        .db
        .get_device(&device_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::UNAUTHORIZED, "Device not found".to_string()))?;

    if dev.is_revoked {
        return Err((StatusCode::UNAUTHORIZED, "Device revoked".to_string()));
    }

    let token = state
        .token_manager
        .issue_token(&device_id, state.config.security.token_ttl_hours)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PairResponse {
        device_id,
        auth_token: token,
    }))
}
