use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use super::{WatchEvent, WatchOptions, Watcher};
use crate::scanner::paths::normalize_key;

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emusic-watch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn emits_scan_request_after_file_change() {
    let root = temp_root("notify");
    let (events_tx, events_rx) = mpsc::channel();
    let options = WatchOptions {
        debounce: Duration::from_millis(200),
        remote_poll_interval: Duration::from_secs(60),
        tick_interval: Duration::from_millis(50),
    };
    let watcher = Watcher::new(options, events_tx).unwrap();
    watcher.set_roots(vec![root.clone()]).unwrap();

    // Give the watcher time to register.
    std::thread::sleep(Duration::from_millis(300));
    std::fs::write(root.join("a.flac"), b"fake flac").unwrap();

    let event = events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("expected a scan request");
    let WatchEvent::ScanRequested { paths } = event;
    assert_eq!(paths.len(), 1);
    assert_eq!(normalize_key(&paths[0]), normalize_key(&root));

    watcher.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn polling_remote_root_emits_scan_request() {
    // Use a UNC path as a remote root; no real share is needed because
    // the watcher only schedules it, it does not access it.
    let remote = PathBuf::from(r"\\fake-nas\music");
    let (events_tx, events_rx) = mpsc::channel();
    let options = WatchOptions {
        debounce: Duration::from_secs(60),
        remote_poll_interval: Duration::from_millis(200),
        tick_interval: Duration::from_millis(50),
    };
    let watcher = Watcher::new(options, events_tx).unwrap();
    watcher.set_roots(vec![remote.clone()]).unwrap();

    let event = events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("expected a remote poll scan request");
    let WatchEvent::ScanRequested { paths } = event;
    assert!(
        paths
            .iter()
            .any(|p| normalize_key(p) == normalize_key(&remote))
    );

    watcher.shutdown().unwrap();
}
