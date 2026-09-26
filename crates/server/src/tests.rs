use std::sync::Arc;
use axum::http::StatusCode;
use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::config::AppConfig;
use crate::db::DbStore;
use crate::util::security::{TokenManager, resolve_path_in_roots};
use crate::api::{AppState, create_router};

#[test]
fn test_token_generation_and_verification() {
    let secret = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let mgr = TokenManager::new(secret);
    let token = mgr.generate_token("device_123", 3600);

    let claims = mgr.verify_token(&token).expect("Token verification failed");
    assert_eq!(claims.device_id, "device_123");
}

#[test]
fn test_invalid_token_signature_rejected() {
    let secret = vec![1, 2, 3, 4];
    let mgr = TokenManager::new(secret);
    let token = mgr.generate_token("device_123", 3600);

    let mut tampered = token.clone();
    tampered.push('a');

    assert!(mgr.verify_token(&tampered).is_err());
}

#[test]
fn test_path_sanitization_and_multi_root_resolution() {
    let temp = std::env::temp_dir();
    let root1 = temp.join("music1");
    let root2 = temp.join("music2");
    std::fs::create_dir_all(&root1).ok();
    std::fs::create_dir_all(&root2).ok();

    let test_file1 = root1.join("song1.mp3");
    let test_file2 = root2.join("song2.mod");
    std::fs::write(&test_file1, b"test1").ok();
    std::fs::write(&test_file2, b"test2").ok();

    let roots = vec![root1.clone(), root2.clone()];

    let res1 = resolve_path_in_roots(&roots, "song1.mp3");
    assert!(res1.is_some());

    let res2 = resolve_path_in_roots(&roots, "song2.mod");
    assert!(res2.is_some());

    let invalid = resolve_path_in_roots(&roots, "../secret.txt");
    assert!(invalid.is_none());

    let _ = std::fs::remove_file(test_file1);
    let _ = std::fs::remove_file(test_file2);
}

#[tokio::test]
async fn test_healthcheck_endpoint() {
    let temp = std::env::temp_dir().join(format!("emusic-test-{}", rand::random::<u64>()));
    let db = Arc::new(DbStore::new(&temp).unwrap());
    let token_mgr = Arc::new(TokenManager::new(vec![1, 2, 3, 4]));
    let config = AppConfig::default();

    let state = Arc::new(AppState { db, token_mgr, config });
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"OK");

    let _ = std::fs::remove_dir_all(temp);
}
