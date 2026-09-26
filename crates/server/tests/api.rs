//! End-to-end API tests: auth, rate limiting, streaming, sync and the path
//! jail, all exercised through the real Axum router.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use emusic_server::auth::paseto::{
    generate_device_keypair, issue_refresh_proof, public_key_paserk, token_fingerprint,
};
use emusic_server::config::{Config, LibraryConfig, SecurityConfig, ServerConfig};
use emusic_server::db::models::NewTrack;
use emusic_server::state::AppState;
use emusic_server::util::unix_now;
use emusic_server::{api, auth};
use http_body_util::BodyExt;
use pasetors::keys::AsymmetricSecretKey;
use pasetors::version4::V4;
use tempfile::TempDir;
use tower::ServiceExt;

struct Harness {
    state: AppState,
    app: Router,
    root: PathBuf,
    _dir: TempDir,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("library");
        std::fs::create_dir_all(&root).unwrap();
        let config = Config {
            server: ServerConfig {
                host: "127.0.0.1".into(),
                port: 0,
                data_dir: dir.path().join("data"),
                trusted_proxies: vec![],
            },
            security: SecurityConfig::default(),
            library: LibraryConfig {
                paths: vec![root.clone()],
                hvsc_songlengths_path: None,
                scan_interval_secs: 0,
            },
        };
        let state = emusic_server::build_state(config).unwrap();
        Self {
            app: api::router(state.clone()),
            state,
            root,
            _dir: dir,
        }
    }

    async fn scan(&self) {
        self.state
            .scan
            .scan_once(self.state.db.clone())
            .await
            .unwrap();
    }

    fn write_wav(&self, name: &str, millis: u32) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        write_wav(&path, millis);
        path
    }

    async fn request(&self, request: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
        let response = self.app.clone().oneshot(request).await.expect("request");
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, body)
    }

    /// Pairs a fresh device and returns `(token, device_id, device_secret)`.
    async fn pair(&self) -> (String, String, AsymmetricSecretKey<V4>) {
        let code = auth::pairing::generate_pairing_code(&self.state.db, 600, unix_now()).unwrap();
        let (secret, public) = generate_device_keypair().unwrap();
        let body = serde_json::json!({
            "pairing_code": code,
            "device_name": "test device",
            "public_key": public_key_paserk(&public).unwrap(),
        });
        let (status, _, response) = self
            .request(json_request("POST", "/api/v1/auth/pair", &body))
            .await;
        assert_eq!(status, StatusCode::OK, "pairing should succeed");
        let value: serde_json::Value = serde_json::from_slice(&response).unwrap();
        (
            value["auth_token"].as_str().unwrap().to_string(),
            value["device_id"].as_str().unwrap().to_string(),
            secret,
        )
    }

    fn authed(&self, method: &str, uri: &str, token: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }
}

fn json_request(method: &str, uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

fn write_wav(path: &Path, millis: u32) {
    let sample_rate: u32 = 8000;
    let samples = sample_rate * millis / 1000;
    let data_len = samples * 2;
    let mut buf = Vec::new();
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVEfmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for index in 0..samples {
        let sample =
            ((index as f64 * 440.0 * 2.0 * PI / sample_rate as f64).sin() * 16000.0) as i16;
        buf.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, buf).unwrap();
}

fn track_row(id: &str, relative_path: &str, album_id: Option<&str>, has_art: bool) -> NewTrack {
    NewTrack {
        id: id.to_string(),
        root_index: 0,
        relative_path: relative_path.to_string(),
        format: "wav".into(),
        kind: "stream".into(),
        title: Some("Injected".into()),
        artist: Some("Artist".into()),
        album_artist: Some("Artist".into()),
        album: Some("Album".into()),
        album_id: album_id.map(str::to_string),
        genre: None,
        year: None,
        track_no: None,
        disc_no: None,
        duration_secs: Some(1.0),
        subtunes: 1,
        channels: Some(1),
        file_size: 100,
        mtime: 1,
        hash: "abc".into(),
        has_art,
        added_at: unix_now(),
    }
}

fn sync_tracks(body: &[u8]) -> Vec<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_slice(body).unwrap();
    value["tracks"].as_array().unwrap().clone()
}

#[tokio::test]
async fn health_is_public() {
    let harness = Harness::new();
    let (status, _, body) = harness
        .request(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["status"], "ok");
}

#[tokio::test]
async fn protected_endpoints_require_a_token() {
    let harness = Harness::new();
    let (status, _, _) = harness
        .request(
            Request::builder()
                .uri("/api/v1/library/sync")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pairing_rejects_a_wrong_code() {
    let harness = Harness::new();
    let real = auth::pairing::generate_pairing_code(&harness.state.db, 600, unix_now()).unwrap();
    let wrong = if real == "111111" { "222222" } else { "111111" };
    let (_, public) = generate_device_keypair().unwrap();
    let body = serde_json::json!({
        "pairing_code": wrong,
        "device_name": "intruder",
        "public_key": public_key_paserk(&public).unwrap(),
    });
    let (status, _, _) = harness
        .request(json_request("POST", "/api/v1/auth/pair", &body))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pairing_grants_access_to_sync() {
    let harness = Harness::new();
    harness.write_wav("a.wav", 500);
    harness.scan().await;
    let (token, _, _) = harness.pair().await;
    let (status, _, body) = harness
        .request(harness.authed("GET", "/api/v1/library/sync?since_version=0", &token))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sync_tracks(&body).len(), 1);
}

#[tokio::test]
async fn pairing_is_rate_limited_per_ip() {
    let harness = Harness::new();
    for attempt in 0..4 {
        let code =
            auth::pairing::generate_pairing_code(&harness.state.db, 600, unix_now()).unwrap();
        let (_, public) = generate_device_keypair().unwrap();
        let body = serde_json::json!({
            "pairing_code": code,
            "device_name": "device",
            "public_key": public_key_paserk(&public).unwrap(),
        });
        let (status, _, _) = harness
            .request(json_request("POST", "/api/v1/auth/pair", &body))
            .await;
        if attempt < 3 {
            assert_eq!(status, StatusCode::OK, "attempt {attempt}");
        } else {
            assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "attempt {attempt}");
        }
    }
}

#[tokio::test]
async fn stream_supports_full_and_range_requests() {
    let harness = Harness::new();
    let path = harness.write_wav("song.wav", 500);
    harness.scan().await;
    let (token, _, _) = harness.pair().await;
    let (_, _, sync) = harness
        .request(harness.authed("GET", "/api/v1/library/sync?since_version=0", &token))
        .await;
    let track = &sync_tracks(&sync)[0];
    let id = track["id"].as_str().unwrap();
    let size = std::fs::metadata(&path).unwrap().len();

    let (status, headers, body) = harness
        .request(harness.authed("GET", &format!("/api/v1/tracks/{id}/stream"), &token))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_LENGTH], size.to_string());
    assert_eq!(body.len() as u64, size);

    let mut range_request = harness.authed("GET", &format!("/api/v1/tracks/{id}/stream"), &token);
    range_request
        .headers_mut()
        .insert(header::RANGE, "bytes=0-9".parse().unwrap());
    let (status, headers, body) = harness.request(range_request).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body.len(), 10);
    assert_eq!(headers[header::CONTENT_RANGE], format!("bytes 0-9/{size}"));
}

#[tokio::test]
async fn specialized_formats_keep_their_content_type() {
    let harness = Harness::new();
    let mut sid = vec![0u8; 0x76];
    sid[0..4].copy_from_slice(b"PSID");
    sid[14..16].copy_from_slice(&3u16.to_be_bytes());
    std::fs::write(harness.root.join("tune.sid"), sid).unwrap();
    harness.scan().await;
    let (token, _, _) = harness.pair().await;
    let (_, _, sync) = harness
        .request(harness.authed("GET", "/api/v1/library/sync?since_version=0", &token))
        .await;
    let tracks = sync_tracks(&sync);
    let sid_track = tracks
        .iter()
        .find(|track| track["format"] == "sid")
        .expect("sid track");
    assert_eq!(sid_track["subtunes"], 3);

    let id = sid_track["id"].as_str().unwrap();
    let (status, headers, _) = harness
        .request(harness.authed("GET", &format!("/api/v1/tracks/{id}/stream"), &token))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "audio/x-sid");
}

#[tokio::test]
async fn sync_reports_additions_updates_and_deletions() {
    let harness = Harness::new();
    harness.write_wav("one.wav", 200);
    harness.scan().await;
    let (token, _, _) = harness.pair().await;

    let (_, _, first) = harness
        .request(harness.authed("GET", "/api/v1/library/sync?since_version=0", &token))
        .await;
    let first: serde_json::Value = serde_json::from_slice(&first).unwrap();
    let v1 = first["version"].as_i64().unwrap();
    let id = first["tracks"][0]["id"].as_str().unwrap().to_string();

    harness.write_wav("two.wav", 300);
    harness.scan().await;
    let (_, _, delta) = harness
        .request(harness.authed(
            "GET",
            &format!("/api/v1/library/sync?since_version={v1}"),
            &token,
        ))
        .await;
    let delta: serde_json::Value = serde_json::from_slice(&delta).unwrap();
    assert_eq!(delta["tracks"].as_array().unwrap().len(), 1);
    let v2 = delta["version"].as_i64().unwrap();

    std::fs::remove_file(harness.root.join("one.wav")).unwrap();
    harness.scan().await;
    let (_, _, delta) = harness
        .request(harness.authed(
            "GET",
            &format!("/api/v1/library/sync?since_version={v2}"),
            &token,
        ))
        .await;
    let delta: serde_json::Value = serde_json::from_slice(&delta).unwrap();
    let deleted = delta["deleted"].as_array().unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].as_str().unwrap(), id);
}

#[tokio::test]
async fn injected_path_traversal_is_rejected() {
    let harness = Harness::new();
    let outside = harness.root.parent().unwrap().join("outside.wav");
    write_wav(&outside, 100);
    harness
        .state
        .db
        .upsert_tracks(&[track_row("evil", "../outside.wav", None, false)])
        .unwrap();
    let (token, _, _) = harness.pair().await;
    let (status, _, _) = harness
        .request(harness.authed("GET", "/api/v1/tracks/evil/stream", &token))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn refresh_requires_a_valid_device_proof() {
    let harness = Harness::new();
    let (token, _, secret) = harness.pair().await;
    let fingerprint = token_fingerprint(&token);
    let proof = issue_refresh_proof(
        &secret,
        &fingerprint,
        std::time::Duration::from_secs(120),
        std::time::Duration::from_secs(30),
    )
    .unwrap();

    let body = serde_json::json!({ "proof": proof });
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let (status, _, response) = harness.request(request).await;
    assert_eq!(status, StatusCode::OK);
    let value: serde_json::Value = serde_json::from_slice(&response).unwrap();
    assert!(
        value["auth_token"]
            .as_str()
            .unwrap()
            .starts_with("v4.public.")
    );

    let bad = serde_json::json!({ "proof": "not-a-token" });
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(&bad).unwrap()))
        .unwrap();
    let (status, _, _) = harness.request(request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revoked_devices_are_locked_out() {
    let harness = Harness::new();
    let (token, device_id, _) = harness.pair().await;
    assert!(harness.state.db.revoke_device(&device_id).unwrap());
    let (status, _, _) = harness
        .request(harness.authed("GET", "/api/v1/library/sync?since_version=0", &token))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn album_art_is_served_from_sidecar_files() {
    let harness = Harness::new();
    harness.write_wav("art/song.wav", 100);
    std::fs::write(harness.root.join("art/cover.png"), b"\x89PNG\r\n\x1a\nfake").unwrap();
    harness
        .state
        .db
        .upsert_tracks(&[track_row("arttrack", "art/song.wav", Some("alb1"), true)])
        .unwrap();
    let (token, _, _) = harness.pair().await;
    let (status, headers, body) = harness
        .request(harness.authed("GET", "/api/v1/albums/alb1/art", &token))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert!(!body.is_empty());

    let (status, _, _) = harness
        .request(harness.authed("GET", "/api/v1/albums/missing/art", &token))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
