//! Integration tests for emusic-server.

use axum::body::Body;
use axum::http::{StatusCode, header};
use std::sync::Arc;
use tower::ServiceExt;

use emusic_server::api::create_router;
use emusic_server::db::models::ServerTrack;
use emusic_server::{AppState, Config, Database, RateLimiter, TokenManager};
use tempfile::tempdir;

fn setup_test_app() -> (
    axum::Router,
    Database,
    TokenManager,
    Config,
    tempfile::TempDir,
) {
    let dir = tempdir().unwrap();
    let music_dir = dir.path().join("music");
    std::fs::create_dir_all(&music_dir).unwrap();

    let mut config = Config::default();
    config.library.paths = vec![music_dir.clone()];

    let db = Database::in_memory().unwrap();
    let (token_manager, _) = TokenManager::generate_random();

    let state = Arc::new(AppState {
        db: db.clone(),
        token_manager: token_manager.clone(),
        rate_limiter: RateLimiter::new(),
        config: config.clone(),
    });

    let router = create_router(state);
    (router, db, token_manager, config, dir)
}

#[tokio::test]
async fn test_pairing_and_auth_flow() {
    let (app, db, _tm, _config, _dir) = setup_test_app();

    // 1. Generate pairing code
    let code = "123456";
    db.add_pairing_code(code, 600).unwrap();

    // 2. Request pair
    let pair_payload = serde_json::json!({
        "pairing_code": code,
        "device_name": "TestDevice",
        "public_key": "pubkey_test"
    });

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/pair")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(pair_payload.to_string()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let pair_res: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let token = pair_res["auth_token"].as_str().unwrap();
    let device_id = pair_res["device_id"].as_str().unwrap();

    assert!(token.starts_with("v4.local."));
    assert!(device_id.starts_with("dev-"));

    // 3. Test protected route with valid token
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/library/sync")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Test device revocation
    db.revoke_device(device_id).unwrap();

    let req_revoked = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/library/sync")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let res_revoked = app.oneshot(req_revoked).await.unwrap();
    assert_eq!(res_revoked.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_stream_route_with_range_and_raw_files() {
    let (app, db, tm, config, _dir) = setup_test_app();
    let music_dir = &config.library.paths[0];

    // Create a standard audio track and a raw SID file
    let mp3_path = music_dir.join("sample.mp3");
    let mp3_data = vec![0u8; 1000];
    std::fs::write(&mp3_path, &mp3_data).unwrap();

    let sid_path = music_dir.join("tune.sid");
    let mut sid_data = vec![0u8; 128];
    sid_data[0..4].copy_from_slice(b"PSID");
    sid_data[14..16].copy_from_slice(&1u16.to_be_bytes());
    std::fs::write(&sid_path, &sid_data).unwrap();

    let mp3_track = ServerTrack {
        id: "mp3_id".to_string(),
        relative_path: "sample.mp3".to_string(),
        format: "mp3".to_string(),
        title: Some("Sample".to_string()),
        artist: None,
        album: None,
        duration_secs: Some(3.0),
        subtunes: 1,
        file_size: 1000,
        mtime: 1000,
        hash: "mp3_id".to_string(),
    };

    let sid_track = ServerTrack {
        id: "sid_id".to_string(),
        relative_path: "tune.sid".to_string(),
        format: "sid".to_string(),
        title: Some("SID Tune".to_string()),
        artist: None,
        album: None,
        duration_secs: None,
        subtunes: 1,
        file_size: 128,
        mtime: 1000,
        hash: "sid_id".to_string(),
    };

    db.upsert_track(&mp3_track).unwrap();
    db.upsert_track(&sid_track).unwrap();

    let dev = db.register_device("dev-test", "TestDev", "pk").unwrap();
    let token = tm.issue_token(&dev.id, 24).unwrap();

    // Range 206 test for standard audio
    let req_range = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/tracks/mp3_id/stream")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::RANGE, "bytes=0-499")
        .body(Body::empty())
        .unwrap();

    let res_range = app.clone().oneshot(req_range).await.unwrap();
    assert_eq!(res_range.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        res_range.headers().get(header::CONTENT_RANGE).unwrap(),
        "bytes 0-499/1000"
    );

    // Direct raw delivery test for SID
    let req_sid = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/tracks/sid_id/stream")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let res_sid = app.oneshot(req_sid).await.unwrap();
    assert_eq!(res_sid.status(), StatusCode::OK);
    assert_eq!(
        res_sid.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/x-sid"
    );
}
