pub mod auth_routes;
pub mod library_routes;
pub mod middleware;
pub mod stream_routes;
pub mod ws;

use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use std::sync::Arc;

use crate::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    let public_routes = Router::new().route("/api/v1/auth/pair", post(auth_routes::pair));

    let protected_routes = Router::new()
        .route("/api/v1/auth/refresh", post(auth_routes::refresh))
        .route("/api/v1/library/sync", get(library_routes::sync_library))
        .route("/api/v1/tracks/{id}/meta", get(library_routes::track_meta))
        .route("/api/v1/albums/{id}/art", get(library_routes::album_art))
        .route(
            "/api/v1/tracks/{id}/stream",
            get(stream_routes::stream_track),
        )
        .route(
            "/api/v1/sid/songlengths",
            get(stream_routes::get_sid_songlengths),
        )
        .route("/api/v1/ws", get(ws::ws_handler))
        .route_layer(from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state)
}
