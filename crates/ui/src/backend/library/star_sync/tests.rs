//! Starred sync against the real `emusic-server` router, served in-process on
//! a loopback socket with a temp database (#516).

use std::collections::HashSet;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;

use emusic_client::auth::{generate_keypair, public_key_paserk};
use emusic_client::{RemoteClient, ServerEndpoint, TrackCache};
use emusic_core::{ArtSource, Track, TrackKind};
use emusic_library::{Store, TrackId};
use emusic_server::config::{Config, LibraryConfig, RenderConfig, SecurityConfig, ServerConfig};
use emusic_server::db::models::NewTrack;
use emusic_server::state::AppState;
use emusic_server::util::unix_now;
use tempfile::TempDir;

use super::{StarSyncOutcome, sync_stars};
use crate::backend::library::remote::apply_delta;
use crate::remote::RemoteServer;

struct Server {
    state: AppState,
    url: String,
    _dir: TempDir,
}

/// Starts a server whose library holds `tracks`, on a loopback port.
fn start_server(tracks: &[NewTrack]) -> Server {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("library");
    std::fs::create_dir_all(&root).expect("library dir");
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
    let state = emusic_server::build_state(config).expect("build state");
    state.db.upsert_tracks(tracks).expect("seed tracks");

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
        url: format!("http://{addr}"),
        _dir: dir,
    }
}

/// Pairs a device, returning the client and its bearer token.
fn pair(server: &Server) -> (RemoteClient, String) {
    let endpoint = ServerEndpoint::new("desktop", &server.url).expect("endpoint");
    let client = RemoteClient::from_endpoint(&endpoint).expect("client");
    let code = emusic_server::auth::pairing::generate_pairing_code(
        &server.state.db,
        &server.state.keys,
        600,
        unix_now(),
    )
    .expect("code");
    let (_, public) = generate_keypair().expect("keypair");
    let response = client
        .pair(
            &code,
            "desktop",
            &public_key_paserk(&public).expect("paserk"),
        )
        .expect("pair");
    (client, response.auth_token)
}

fn server_track(id: &str, artist: &str, album: &str, title: &str, no: u32) -> NewTrack {
    NewTrack {
        id: id.to_string(),
        root_index: 0,
        relative_path: format!("{id}.flac"),
        format: "flac".into(),
        kind: "stream".into(),
        title: Some(title.into()),
        artist: Some(artist.into()),
        album_artist: None,
        album: Some(album.into()),
        album_id: None,
        genre: None,
        year: None,
        track_no: Some(no),
        disc_no: None,
        duration_secs: Some(1.0),
        subtunes: 1,
        channels: Some(2),
        file_size: 100,
        mtime: 1,
        hash: format!("hash-{id}"),
        has_art: false,
        added_at: unix_now(),
    }
}

/// A local library file with the given tags.
fn local_track(path: &str, artist: &str, album: &str, title: &str, no: u32) -> Track {
    Track {
        id: TrackId::UNASSIGNED,
        path: PathBuf::from(path),
        dir: PathBuf::from("music"),
        filename: path.to_string(),
        ext: "flac".into(),
        size: 1,
        mtime: 1,
        kind: TrackKind::Stream,
        duration_ms: 1000,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: Some(title.into()),
        artist: Some(artist.into()),
        album_artist: None,
        album: Some(album.into()),
        genre: None,
        year: None,
        track_no: Some(no),
        disc_no: None,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: 1,
        starred: false,
    }
}

fn server_starred(client: &RemoteClient, token: &str) -> HashSet<String> {
    let set = client
        .starred(token, None)
        .expect("starred")
        .expect("unconditional fetch returns the set");
    set.tracks.into_iter().map(|track| track.id).collect()
}

fn starred(store: &Store, id: TrackId) -> bool {
    store
        .load_all_tracks()
        .expect("tracks")
        .into_iter()
        .find(|track| track.id == id)
        .expect("track exists")
        .starred
}

fn ids(items: &[&str]) -> HashSet<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn stars_sync_both_ways_including_metadata_matched_local_tracks() {
    let server = start_server(&[
        server_track("aa01", "Björk", "Debut", "Human Behaviour", 1),
        server_track("bb02", "Artist", "Album", "Other Song", 2),
    ]);
    let (client, token) = pair(&server);
    let remote = RemoteServer::new("Home", &server.url).expect("server entry");
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache = TrackCache::with_root(cache_dir.path().to_path_buf());

    let mut store = Store::open_in_memory().expect("store");
    let mut local = vec![
        // Matches aa01 by metadata despite case and accents.
        local_track("human.flac", "bjork", "DEBUT", "human behaviour", 1),
        // Matches nothing on the server.
        local_track("unknown.flac", "Nobody", "Nowhere", "Nothing", 1),
    ];
    store.upsert_tracks(&mut local).expect("local tracks");
    let (local_human, local_unknown) = (local[0].id, local[1].id);
    assert_eq!(
        apply_delta(&mut store, &client, &token, &remote, &cache).expect("delta"),
        2
    );
    let remote_id = |remote_track: &str| {
        store
            .star_sync_rows(&remote.id)
            .expect("rows")
            .into_iter()
            .find(|row| row.remote_track_id.as_deref() == Some(remote_track))
            .expect("remote row")
            .id
    };
    let (remote_human, remote_other) = (remote_id("aa01"), remote_id("bb02"));

    // Star on the desktop: a remote row by id, the unmatched local file too.
    store.set_starred(remote_other, true).expect("star");
    store.set_starred(local_unknown, true).expect("star");
    let outcome = sync_stars(&mut store, &client, &token, &remote.id).expect("sync");
    assert_eq!(outcome.pushed, 1);
    assert_eq!(server_starred(&client, &token), ids(&["bb02"]));

    // Star on the server (e.g. from another desktop): both the remote row and
    // the local file matched by metadata get starred.
    client.star(&token, "aa01").expect("server star");
    let outcome = sync_stars(&mut store, &client, &token, &remote.id).expect("sync");
    assert_eq!(
        outcome,
        StarSyncOutcome {
            pushed: 0,
            applied: 2
        }
    );
    assert!(starred(&store, remote_human));
    assert!(starred(&store, local_human));

    // Nothing changed: the conditional fetch is a 304 and nothing moves.
    let outcome = sync_stars(&mut store, &client, &token, &remote.id).expect("sync");
    assert_eq!(outcome, StarSyncOutcome::default());

    // Unstar the metadata-matched local file on the desktop: the server
    // drops it and the remote row of the same song follows.
    store.set_starred(local_human, false).expect("unstar");
    let outcome = sync_stars(&mut store, &client, &token, &remote.id).expect("sync");
    assert_eq!(
        outcome,
        StarSyncOutcome {
            pushed: 1,
            applied: 1
        }
    );
    assert_eq!(server_starred(&client, &token), ids(&["bb02"]));
    assert!(!starred(&store, remote_human));
    assert!(starred(&store, local_unknown), "unmatched stars stay local");

    // Unstar on the server: applied on the desktop.
    client.unstar(&token, "bb02").expect("server unstar");
    sync_stars(&mut store, &client, &token, &remote.id).expect("sync");
    assert!(!starred(&store, remote_other));
    assert!(server_starred(&client, &token).is_empty());
}
