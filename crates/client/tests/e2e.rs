//! End-to-end tests: the client crate against a real `emusic-server` router
//! served over a loopback TCP socket.

use std::f64::consts::PI;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use emusic_client::auth::{
    generate_keypair, issue_refresh_proof, public_key_paserk, secret_key_paserk, token_fingerprint,
};
use emusic_client::{ClientError, Credentials, RemoteClient, ServerEndpoint, TrackCache};
use emusic_server::config::{Config, LibraryConfig, SecurityConfig, ServerConfig};
use emusic_server::state::AppState;
use emusic_server::util::unix_now;
use tempfile::TempDir;

struct Server {
    state: AppState,
    addr: SocketAddr,
    _dir: TempDir,
}

/// Starts a scanned server on a loopback port and returns it.
fn start_server() -> Server {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("library");
    std::fs::create_dir_all(&root).expect("library dir");
    write_wav(&root.join("song.wav"), 500);

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
    };
    let state = emusic_server::build_state(config).expect("build state");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime
        .block_on(state.scan.scan_once(state.db.clone()))
        .expect("scan");
    drop(runtime);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(true).expect("nonblocking");
    let addr = listener.local_addr().expect("addr");
    let app = emusic_server::api::router(state.clone());
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("server runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("listener");
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .expect("serve");
        });
    });

    Server {
        state,
        addr,
        _dir: dir,
    }
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
    std::fs::write(path, buf).expect("write wav");
}

struct Pairing {
    endpoint: ServerEndpoint,
    client: RemoteClient,
    credentials: Credentials,
}

fn pair(server: &Server) -> Pairing {
    let endpoint =
        ServerEndpoint::new("test", &format!("http://{}", server.addr)).expect("endpoint");
    let client = RemoteClient::from_endpoint(&endpoint).expect("client");
    let code = emusic_server::auth::pairing::generate_pairing_code(
        &server.state.db,
        &server.state.keys,
        600,
        unix_now(),
    )
    .expect("code");
    let (secret, public) = generate_keypair().expect("keypair");
    let response = client
        .pair(
            &code,
            "test device",
            &public_key_paserk(&public).expect("paserk"),
        )
        .expect("pair");
    let credentials = Credentials {
        device_id: response.device_id,
        device_name: response.device_name,
        secret: secret_key_paserk(&secret).expect("secret paserk"),
        token: response.auth_token,
        expires_at: response.expires_at,
        since_version: 0,
    };
    Pairing {
        endpoint,
        client,
        credentials,
    }
}

#[test]
fn pair_sync_download_and_meta() {
    let server = start_server();
    let pairing = pair(&server);

    let delta = pairing
        .client
        .sync(&pairing.credentials.token, 0)
        .expect("sync");
    assert_eq!(delta.tracks.len(), 1);
    let track = &delta.tracks[0];
    assert_eq!(track.format, "wav");
    assert!(!track.specialized);

    let meta = pairing
        .client
        .track_meta(&pairing.credentials.token, &track.id)
        .expect("meta");
    assert_eq!(meta.id, track.id);

    let mut bytes = Vec::new();
    let copied = pairing
        .client
        .download_to(&pairing.credentials.token, &track.id, &mut bytes)
        .expect("download");
    assert_eq!(copied as usize, bytes.len());
    assert_eq!(bytes.len() as u64, track.file_size);

    let songlengths = pairing
        .client
        .songlengths(&pairing.credentials.token)
        .expect("songlengths");
    assert!(songlengths.is_empty());
}

#[test]
fn cache_downloads_once_and_reuses_the_file() {
    let server = start_server();
    let pairing = pair(&server);
    let delta = pairing
        .client
        .sync(&pairing.credentials.token, 0)
        .expect("sync");
    let track = &delta.tracks[0];

    let dir = tempfile::tempdir().expect("cache dir");
    let cache = TrackCache::with_root(dir.path().to_path_buf());
    let first = cache
        .ensure(
            &pairing.client,
            &pairing.credentials.token,
            &pairing.endpoint.id,
            track,
        )
        .expect("ensure");
    assert!(first.is_file());
    assert_eq!(std::fs::metadata(&first).unwrap().len(), track.file_size);

    let second = cache
        .ensure(
            &pairing.client,
            &pairing.credentials.token,
            &pairing.endpoint.id,
            track,
        )
        .expect("ensure again");
    assert_eq!(first, second);
}

#[test]
fn refresh_proof_renews_the_token() {
    let server = start_server();
    let pairing = pair(&server);
    let fingerprint = token_fingerprint(&pairing.credentials.token);
    let proof = issue_refresh_proof(
        &pairing.credentials.secret_key().expect("secret"),
        &fingerprint,
        Duration::from_secs(120),
        Duration::from_secs(30),
    )
    .expect("proof");
    let response = pairing
        .client
        .refresh(&pairing.credentials.token, &proof)
        .expect("refresh");
    assert!(response.auth_token.starts_with("v4.public."));
    pairing
        .client
        .sync(&response.auth_token, 0)
        .expect("new token works");
}

#[test]
fn revoked_device_is_rejected() {
    let server = start_server();
    let pairing = pair(&server);
    server
        .state
        .db
        .revoke_device(&pairing.credentials.device_id)
        .expect("revoke");
    let error = pairing
        .client
        .sync(&pairing.credentials.token, 0)
        .expect_err("must fail");
    assert!(matches!(error, ClientError::Unauthorized));
}

#[test]
fn refresh_proof_bound_to_another_token_is_rejected() {
    let server = start_server();
    let pairing = pair(&server);
    let proof = issue_refresh_proof(
        &pairing.credentials.secret_key().expect("secret"),
        &token_fingerprint("a different token"),
        Duration::from_secs(120),
        Duration::from_secs(30),
    )
    .expect("proof");
    let error = pairing
        .client
        .refresh(&pairing.credentials.token, &proof)
        .expect_err("must fail");
    assert!(matches!(error, ClientError::Unauthorized));
}

#[test]
fn cached_file_is_reused_without_contacting_the_server() {
    let server = start_server();
    let pairing = pair(&server);
    let delta = pairing
        .client
        .sync(&pairing.credentials.token, 0)
        .expect("sync");
    let track = &delta.tracks[0];

    let dir = tempfile::tempdir().expect("cache dir");
    let cache = TrackCache::with_root(dir.path().to_path_buf());
    let path = cache.path_for(&pairing.endpoint.id, track).expect("path");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, vec![0u8; track.file_size as usize]).unwrap();

    // A client pointing at a dead port proves no network call is made.
    let dead = RemoteClient::new("http://127.0.0.1:1").expect("client");
    let reused = cache
        .ensure(&dead, "unused", &pairing.endpoint.id, track)
        .expect("reused");
    assert_eq!(reused, path);
}

#[test]
fn concurrent_cache_fetches_do_not_corrupt_the_file() {
    let server = start_server();
    let pairing = pair(&server);
    let delta = pairing
        .client
        .sync(&pairing.credentials.token, 0)
        .expect("sync");
    let track = Arc::new(delta.tracks[0].clone());

    let dir = tempfile::tempdir().expect("cache dir");
    let cache = Arc::new(TrackCache::with_root(dir.path().to_path_buf()));
    let client = Arc::new(pairing.client.clone());
    let endpoint = Arc::new(pairing.endpoint.clone());
    let token = Arc::new(pairing.credentials.token.clone());

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let cache = Arc::clone(&cache);
            let client = Arc::clone(&client);
            let endpoint = Arc::clone(&endpoint);
            let token = Arc::clone(&token);
            let track = Arc::clone(&track);
            std::thread::spawn(move || {
                cache
                    .ensure(&client, &token, &endpoint.id, &track)
                    .expect("ensure")
            })
        })
        .collect();

    let mut paths = Vec::new();
    for handle in handles {
        paths.push(handle.join().expect("join"));
    }
    assert!(paths.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(std::fs::metadata(&paths[0]).unwrap().len(), track.file_size);
}
