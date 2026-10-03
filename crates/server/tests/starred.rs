//! Starred-track sync API tests (#513), through the real Axum router.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use emusic_server::config::{Config, LibraryConfig, RenderConfig, SecurityConfig, ServerConfig};
use emusic_server::db::models::NewTrack;
use emusic_server::scan::ServerEvent;
use emusic_server::state::AppState;
use emusic_server::util::unix_now;
use emusic_server::{api, auth};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

struct Harness {
    state: AppState,
    app: Router,
    token: String,
    _dir: TempDir,
}

impl Harness {
    /// A server with tracks `a`, `b` and `c` and one paired device.
    async fn new() -> Self {
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
                paths: vec![root],
                hvsc_songlengths_path: None,
                scan_interval_secs: 0,
            },
            render: RenderConfig::default(),
            ..Config::default()
        };
        let state = emusic_server::build_state(config).unwrap();
        state
            .db
            .upsert_tracks(&[track("a"), track("b"), track("c")])
            .unwrap();
        let app = api::router(state.clone());
        let token = pair(&state, &app).await;
        Self {
            state,
            app,
            token,
            _dir: dir,
        }
    }

    async fn send(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let (status, _, bytes) = self.send_with(method, uri, body, None).await;
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, value)
    }

    async fn send_with(
        &self,
        method: &str,
        uri: &str,
        body: Option<Value>,
        if_none_match: Option<&str>,
    ) -> (StatusCode, HeaderMap, Bytes) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token));
        if let Some(tag) = if_none_match {
            builder = builder.header(header::IF_NONE_MATCH, tag);
        }
        let request = match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap())),
            None => builder.body(Body::empty()),
        }
        .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, bytes)
    }

    async fn starred_ids(&self) -> (i64, Vec<String>) {
        let (status, body) = self.send("GET", "/api/v1/starred", None).await;
        assert_eq!(status, StatusCode::OK);
        let ids = body["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|track| track["id"].as_str().unwrap().to_string())
            .collect();
        (body["version"].as_i64().unwrap(), ids)
    }
}

async fn pair(state: &AppState, app: &Router) -> String {
    let code =
        auth::pairing::generate_pairing_code(&state.db, &state.keys, 600, unix_now()).unwrap();
    let (_, public) = auth::paseto::generate_device_keypair().unwrap();
    let body = json!({
        "pairing_code": code,
        "device_name": "test device",
        "public_key": auth::paseto::public_key_paserk(&public).unwrap(),
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/pair")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    value["auth_token"].as_str().unwrap().to_string()
}

fn track(id: &str) -> NewTrack {
    NewTrack {
        id: id.to_string(),
        root_index: 0,
        relative_path: format!("{id}.wav"),
        format: "wav".into(),
        kind: "stream".into(),
        title: Some(id.into()),
        artist: None,
        album_artist: None,
        album: None,
        album_id: None,
        genre: None,
        year: None,
        track_no: None,
        disc_no: None,
        duration_secs: Some(1.0),
        subtunes: 1,
        channels: Some(1),
        file_size: 100,
        mtime: 1,
        hash: format!("hash-{id}"),
        has_art: false,
        added_at: unix_now(),
    }
}

#[tokio::test]
async fn starred_requires_a_token() {
    let harness = Harness::new().await;
    let request = Request::builder()
        .uri("/api/v1/starred")
        .body(Body::empty())
        .unwrap();
    let response = harness.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn star_and_unstar_are_idempotent_and_bump_the_version_on_change() {
    let harness = Harness::new().await;
    assert_eq!(harness.starred_ids().await, (0, vec![]));

    let (status, body) = harness.send("PUT", "/api/v1/starred/a", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "version": 1 }));
    // Starring again changes nothing.
    let (_, body) = harness.send("PUT", "/api/v1/starred/a", None).await;
    assert_eq!(body, json!({ "version": 1 }));

    harness.send("PUT", "/api/v1/starred/b", None).await;
    let (version, ids) = harness.starred_ids().await;
    assert_eq!(version, 2);
    // Newest first.
    assert_eq!(ids, vec!["b", "a"]);

    let (status, body) = harness.send("DELETE", "/api/v1/starred/a", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "version": 3 }));
    let (_, body) = harness.send("DELETE", "/api/v1/starred/a", None).await;
    assert_eq!(body, json!({ "version": 3 }));
    // Unstarring an id that was never starred (or never existed) is fine.
    let (status, body) = harness.send("DELETE", "/api/v1/starred/zzz", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "version": 3 }));

    assert_eq!(harness.starred_ids().await, (3, vec!["b".to_string()]));
}

#[tokio::test]
async fn starring_an_unknown_track_is_not_found() {
    let harness = Harness::new().await;
    let (status, _) = harness.send("PUT", "/api/v1/starred/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(harness.starred_ids().await, (0, vec![]));
}

#[tokio::test]
async fn batch_applies_in_one_version_and_reports_unknown_ids() {
    let harness = Harness::new().await;
    harness.send("PUT", "/api/v1/starred/c", None).await;

    let (status, body) = harness
        .send(
            "POST",
            "/api/v1/starred/batch",
            Some(json!({ "star": ["a", "b", "ghost", "a"], "unstar": ["c", "never"] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "version": 2, "unknown": ["ghost"] }));
    let (version, mut ids) = harness.starred_ids().await;
    ids.sort();
    assert_eq!((version, ids), (2, vec!["a".to_string(), "b".to_string()]));

    // A no-op batch keeps the version.
    let (_, body) = harness
        .send(
            "POST",
            "/api/v1/starred/batch",
            Some(json!({ "star": ["a"], "unstar": [] })),
        )
        .await;
    assert_eq!(body, json!({ "version": 2, "unknown": [] }));
}

#[tokio::test]
async fn batch_rejects_more_than_the_id_limit() {
    let harness = Harness::new().await;
    // 64-hex ids, like real track ids, also exercise the batch body limit.
    let ids: Vec<String> = (0..5_001).map(|i| format!("{i:064x}")).collect();
    let (status, _) = harness
        .send(
            "POST",
            "/api/v1/starred/batch",
            Some(json!({ "star": ids[..2_500], "unstar": ids[2_500..] })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, body) = harness
        .send(
            "POST",
            "/api/v1/starred/batch",
            Some(json!({ "star": ids[..5_000], "unstar": [] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["unknown"].as_array().unwrap().len(), 5_000);
    assert_eq!(body["version"], 0);
}

#[tokio::test]
async fn list_honours_if_none_match() {
    let harness = Harness::new().await;
    harness.send("PUT", "/api/v1/starred/a", None).await;

    let (status, headers, _) = harness
        .send_with("GET", "/api/v1/starred", None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::ETAG], "\"1\"");

    let (status, headers, body) = harness
        .send_with("GET", "/api/v1/starred", None, Some("\"1\""))
        .await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert_eq!(headers[header::ETAG], "\"1\"");
    assert!(body.is_empty());

    harness.send("PUT", "/api/v1/starred/b", None).await;
    let (status, _, _) = harness
        .send_with("GET", "/api/v1/starred", None, Some("\"1\""))
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_deleted_track_is_hidden_but_keeps_its_star() {
    let harness = Harness::new().await;
    harness.send("PUT", "/api/v1/starred/a", None).await;
    harness.send("PUT", "/api/v1/starred/b", None).await;

    harness.state.db.delete_tracks(&["a".to_string()]).unwrap();
    assert_eq!(harness.starred_ids().await, (2, vec!["b".to_string()]));

    // The file comes back: its star is still there.
    harness.state.db.upsert_tracks(&[track("a")]).unwrap();
    assert_eq!(
        harness.starred_ids().await,
        (2, vec!["b".to_string(), "a".to_string()])
    );
}

#[tokio::test]
async fn changes_are_broadcast_to_websocket_subscribers() {
    let harness = Harness::new().await;
    let mut events = harness.state.scan.subscribe();
    harness.send("PUT", "/api/v1/starred/a", None).await;
    // A no-op broadcasts nothing.
    harness.send("PUT", "/api/v1/starred/a", None).await;
    harness.send("DELETE", "/api/v1/starred/a", None).await;

    let versions: Vec<i64> = std::iter::from_fn(|| events.try_recv().ok())
        .map(|event| match event {
            ServerEvent::StarredChanged { version } => version,
            other => panic!("unexpected event {other:?}"),
        })
        .collect();
    assert_eq!(versions, vec![1, 2]);
    let wire = serde_json::to_value(ServerEvent::StarredChanged { version: 2 }).unwrap();
    assert_eq!(wire, json!({ "type": "starred_changed", "version": 2 }));
}
