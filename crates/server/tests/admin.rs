//! Admin listener tests, driven through the real admin router with
//! `tower::ServiceExt::oneshot`.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use emusic_server::audit::{AuditQuery, AuditRow};
use emusic_server::auth::paseto::{generate_device_keypair, public_key_paserk};
use emusic_server::config::{AdminConfig, Config, LibraryConfig, ServerConfig};
use emusic_server::state::AppState;
use emusic_server::util::unix_now;
use emusic_server::{admin, api, auth};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

const TOKEN: &str = "an-admin-token-that-is-long-enough";

struct Harness {
    state: AppState,
    admin: Router,
    public: Router,
    _dir: TempDir,
}

impl Harness {
    fn new(token: &str) -> Self {
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
            library: LibraryConfig {
                paths: vec![root],
                hvsc_songlengths_path: None,
                scan_interval_secs: 0,
            },
            admin: AdminConfig {
                token: token.to_string(),
                ..AdminConfig::default()
            },
            ..Config::default()
        };
        let state = emusic_server::build_state(config).unwrap();
        Self {
            admin: admin::router(state.clone()),
            public: api::router(state.clone()),
            state,
            _dir: dir,
        }
    }

    async fn send(router: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
        let response = router.clone().oneshot(request).await.expect("request");
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, body)
    }

    async fn admin_get(&self, uri: &str) -> (StatusCode, HeaderMap, Bytes) {
        Self::send(&self.admin, get(uri).body(Body::empty()).unwrap()).await
    }

    async fn admin_post(&self, uri: &str, form: &str) -> (StatusCode, HeaderMap, Bytes) {
        let request = Request::post(uri)
            .header(header::HOST, "127.0.0.1:8081")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_string()))
            .unwrap();
        Self::send(&self.admin, request).await
    }

    /// Pairs a device through the public API; returns `(token, device_id)`.
    async fn pair(&self) -> (String, String) {
        let code =
            auth::pairing::generate_pairing_code(&self.state.db, &self.state.keys, 600, unix_now())
                .unwrap();
        let (_secret, public) = generate_device_keypair().unwrap();
        let body = serde_json::json!({
            "pairing_code": code,
            "device_name": "admin test device",
            "public_key": public_key_paserk(&public).unwrap(),
        });
        let request = Request::post("/api/v1/auth/pair")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let (status, _, response) = Self::send(&self.public, request).await;
        assert_eq!(status, StatusCode::OK);
        let value: serde_json::Value = serde_json::from_slice(&response).unwrap();
        (
            value["auth_token"].as_str().unwrap().to_string(),
            value["device_id"].as_str().unwrap().to_string(),
        )
    }

    fn audit_rows(&self, event: &str) -> Vec<AuditRow> {
        self.state.audit.flush();
        self.state
            .db
            .audit_rows(&AuditQuery {
                event: Some(event.to_string()),
                limit: 100,
                ..AuditQuery::default()
            })
            .unwrap()
    }
}

fn get(uri: &str) -> axum::http::request::Builder {
    Request::get(uri).header(header::HOST, "127.0.0.1:8081")
}

fn basic(password: &str) -> String {
    format!("Basic {}", STANDARD.encode(format!("admin:{password}")))
}

fn json(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).expect("json body")
}

/// Pulls the CSRF token out of a rendered form.
fn csrf_token(html: &str) -> String {
    let marker = "name=\"csrf\" value=\"";
    let start = html.find(marker).expect("csrf field") + marker.len();
    let end = html[start..].find('"').unwrap() + start;
    html[start..end].to_string()
}

#[tokio::test]
async fn public_router_has_no_admin_routes() {
    let harness = Harness::new("");
    for uri in ["/admin", "/admin/", "/admin/api/overview", "/admin/devices"] {
        let (status, _, _) =
            Harness::send(&harness.public, get(uri).body(Body::empty()).unwrap()).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{uri} must not exist publicly"
        );
    }
}

#[tokio::test]
async fn a_configured_token_is_required() {
    let harness = Harness::new(TOKEN);
    let (status, headers, _) = harness.admin_get("/admin/api/overview").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        headers[header::WWW_AUTHENTICATE]
            .to_str()
            .unwrap()
            .starts_with("Basic ")
    );
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");

    let wrong = get("/admin/api/overview")
        .header(header::AUTHORIZATION, basic("not-the-token"))
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = Harness::send(&harness.admin, wrong).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let failures = harness.audit_rows("admin_auth_failed");
    assert_eq!(failures.len(), 1, "only the wrong password is audited");
    assert_eq!(failures[0].detail["reason"], "wrong");

    let right = get("/admin/api/overview")
        .header(header::AUTHORIZATION, basic(TOKEN))
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = Harness::send(&harness.admin, right).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn repeated_failed_logins_are_rate_limited() {
    let harness = Harness::new(TOKEN);
    for _ in 0..admin::auth::AUTH_FAILURE_LIMIT {
        let wrong = get("/admin/")
            .header(header::AUTHORIZATION, basic("guess"))
            .body(Body::empty())
            .unwrap();
        let (status, _, _) = Harness::send(&harness.admin, wrong).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let right = get("/admin/")
        .header(header::AUTHORIZATION, basic(TOKEN))
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = Harness::send(&harness.admin, right).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(harness.audit_rows("rate_limited").len(), 1);
}

#[tokio::test]
async fn without_a_token_only_local_host_headers_are_served() {
    let harness = Harness::new("");
    let (status, _, _) = harness.admin_get("/admin/").await;
    assert_eq!(status, StatusCode::OK);
    let rebound = Request::get("/admin/")
        .header(header::HOST, "evil.example:8081")
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = Harness::send(&harness.admin, rebound).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn overview_json_has_the_expected_shape() {
    let harness = Harness::new("");
    let (status, headers, body) = harness.admin_get("/admin/api/overview").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY");
    assert!(
        headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("default-src 'none'")
    );
    let overview = json(&body);
    assert_eq!(overview["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        overview["schema_version"],
        emusic_server::db::schema::CURRENT_VERSION
    );
    assert!(overview["library_version"].is_i64());
    assert!(overview["uptime_secs"].is_i64());
    assert!(overview["files"]["db_bytes"].as_u64().unwrap() > 0);
    assert_eq!(overview["roots"].as_array().unwrap().len(), 1);
    assert_eq!(overview["scan"]["running"], false);
    assert!(overview["render_cache_bytes"].is_null());
    let tables: Vec<&str> = overview["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|table| table["table"].as_str().unwrap())
        .collect();
    for expected in ["audit_log", "devices", "pairing_codes", "tracks"] {
        assert!(tables.contains(&expected), "missing {expected}");
    }
}

#[tokio::test]
async fn every_page_renders() {
    let harness = Harness::new("");
    harness.pair().await;
    for uri in [
        "/admin/",
        "/admin/active",
        "/admin/devices",
        "/admin/pairing",
        "/admin/audit",
        "/admin/audit?event=device_paired&since=2020-01-01",
        "/admin/database",
        "/admin/api/active",
        "/admin/api/devices",
        "/admin/api/pairing",
        "/admin/api/audit",
        "/admin/api/database",
    ] {
        let (status, _, _) = harness.admin_get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
    }
    let (status, _, _) = harness.admin_get("/admin/audit?since=garbage").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn revoking_from_the_admin_page_is_audited() {
    let harness = Harness::new("");
    let (token, device_id) = harness.pair().await;
    let (_, _, page) = harness.admin_get("/admin/devices").await;
    let page = std::str::from_utf8(&page).unwrap();
    assert!(page.contains(&device_id));
    let csrf = csrf_token(page);
    let action = format!("/admin/devices/{device_id}/revoke");

    let (status, _, _) = harness.admin_post(&action, "csrf=forged").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a bad form token is refused");
    assert!(
        !harness
            .state
            .db
            .device_by_id(&device_id)
            .unwrap()
            .unwrap()
            .is_revoked
    );

    let (status, headers, _) = harness.admin_post(&action, &format!("csrf={csrf}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], "/admin/devices");
    assert!(
        harness
            .state
            .db
            .device_by_id(&device_id)
            .unwrap()
            .unwrap()
            .is_revoked
    );

    let revoked = harness.audit_rows("device_revoked");
    assert_eq!(revoked.len(), 1);
    assert_eq!(revoked[0].device_id.as_deref(), Some(device_id.as_str()));
    assert_eq!(revoked[0].detail["by"], "admin");

    let sync = Request::get("/api/v1/library/sync")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = Harness::send(&harness.public, sync).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the revoked device is locked out"
    );
}

#[tokio::test]
async fn active_users_reflect_authenticated_requests() {
    let harness = Harness::new("");
    let (token, device_id) = harness.pair().await;
    let (_, _, body) = harness.admin_get("/admin/api/active").await;
    assert!(json(&body).as_array().unwrap().is_empty());

    let sync = Request::get("/api/v1/library/sync")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::USER_AGENT, "emusic-test/1.0")
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = Harness::send(&harness.public, sync).await;
    assert_eq!(status, StatusCode::OK);

    let (_, _, body) = harness.admin_get("/admin/api/active").await;
    let active = json(&body);
    assert_eq!(active[0]["device_id"], device_id.as_str());
    assert_eq!(active[0]["user_agent"], "emusic-test/1.0");
    assert_eq!(active[0]["open_websockets"], 0);
}

#[tokio::test]
async fn generating_a_code_from_the_admin_page_is_audited() {
    let harness = Harness::new("");
    let (_, _, page) = harness.admin_get("/admin/pairing").await;
    let csrf = csrf_token(std::str::from_utf8(&page).unwrap());
    let (status, _, body) = harness
        .admin_post("/admin/pairing/codes", &format!("csrf={csrf}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("class=\"code\"")
    );
    let created = harness.audit_rows("pairing_code_created");
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].detail["by"], "admin");

    let (_, _, body) = harness.admin_get("/admin/api/pairing").await;
    assert_eq!(json(&body)["codes"]["outstanding"], 1);
}

#[tokio::test]
async fn pairing_failures_record_their_reason() {
    let harness = Harness::new("");
    let code = auth::pairing::generate_pairing_code(
        &harness.state.db,
        &harness.state.keys,
        600,
        unix_now(),
    )
    .unwrap();
    let wrong = if code == "000000" { "000001" } else { "000000" };
    let (_secret, public) = generate_device_keypair().unwrap();
    let key = public_key_paserk(&public).unwrap();
    for attempt in [wrong, code.as_str(), code.as_str()] {
        let body = serde_json::json!({
            "pairing_code": attempt, "device_name": "x", "public_key": key,
        });
        let request = Request::post("/api/v1/auth/pair")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        Harness::send(&harness.public, request).await;
    }
    let reasons: Vec<_> = harness
        .audit_rows("pair_failed")
        .iter()
        .map(|row| row.detail["reason"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(reasons, ["reused", "wrong"], "newest first");
    assert_eq!(harness.audit_rows("device_paired").len(), 1);
}
