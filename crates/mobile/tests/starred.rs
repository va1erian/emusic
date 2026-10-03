//! `MobileCore::refresh_starred` / `cached_starred` against a scripted HTTP
//! server: the fetch is conditional on the cached version, the result follows
//! the server's (newest-first) order and is limited to the local library.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

use emusic_client::{CredentialStore, Credentials, ServerEndpoint, unix_now};
use emusic_mobile::MobileCore;

/// One library row in the snapshot layout `library.json` uses.
fn row(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","filename":"{id}.mp3","directory":"","format":"mp3","kind":"stream",
        "specialized":false,"title":"{id}","artist":null,"album_artist":null,"album":null,
        "album_id":null,"genre":null,"year":null,"track_no":null,"disc_no":null,
        "duration_secs":null,"subtunes":1,"channels":null,"file_size":0,"has_art":false,
        "sync_version":1,"added_at":0}}"#
    )
}

/// Serves `responses` in order, one per connection, and reports each request's
/// `If-None-Match` header (empty when absent).
fn scripted_server(responses: Vec<String>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut etag = String::new();
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(
                line.starts_with("GET /api/v1/starred "),
                "unexpected {line}"
            );
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("if-none-match")
                {
                    etag = value.trim().to_string();
                }
            }
            sender.send(etag).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (url, receiver)
}

fn ok(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )
}

const NOT_MODIFIED: &str =
    "HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

#[test]
fn refresh_is_conditional_ordered_and_filtered_to_the_library() {
    let set = r#"{"version":3,"tracks":[{"id":"c","starred_at":30},
        {"id":"gone","starred_at":20},{"id":"a","starred_at":10}]}"#;
    let (url, etags) = scripted_server(vec![ok(set), NOT_MODIFIED.to_string()]);

    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    std::fs::write(
        data.join("library.json"),
        format!(
            r#"{{"schema":2,"version":1,"tracks":[{},{},{}]}}"#,
            row("a"),
            row("b"),
            row("c")
        ),
    )
    .unwrap();
    // A token far from expiry, so no refresh round-trip is attempted.
    let endpoint = ServerEndpoint::new("server", &url).unwrap();
    CredentialStore::with_dir(data.join("servers"))
        .save(
            &endpoint.id,
            &Credentials {
                device_id: "device".into(),
                device_name: "phone".into(),
                secret: String::new(),
                token: "token".into(),
                expires_at: unix_now() + 30 * 24 * 3600,
                since_version: 1,
            },
        )
        .unwrap();

    let core = MobileCore::new(url, data.to_string_lossy().into_owned()).unwrap();
    assert!(core.cached_starred().unwrap().is_empty());

    // First fetch: unconditional, newest first, unknown track dropped.
    assert_eq!(core.refresh_starred().unwrap(), ["c", "a"]);
    assert_eq!(etags.recv().unwrap(), "");
    assert_eq!(core.cached_starred().unwrap(), ["c", "a"]);

    // Second fetch: conditional on version 3; a 304 keeps the cached set.
    assert_eq!(core.refresh_starred().unwrap(), ["c", "a"]);
    assert_eq!(etags.recv().unwrap(), "\"3\"");
}
