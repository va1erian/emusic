//! Integration tests for single-instance detection and IPC batching.
//!
//! Every test uses a unique `app_id` (pipe name), so tests never collide
//! with each other or with a real running emusic.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use winshell::instance::send_to_primary;
use winshell::{IpcMessage, SingleInstance};

fn unique_app_id(case: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("emusic-itest-{case}-{}-{nanos}", std::process::id())
}

fn message(files: &[&str], enqueue: bool) -> IpcMessage {
    IpcMessage {
        enqueue,
        files: files.iter().map(PathBuf::from).collect(),
        cwd: PathBuf::from("."),
    }
}

#[test]
fn second_acquire_for_same_app_id_becomes_secondary() {
    let app_id = unique_app_id("second-acquire");

    let primary = SingleInstance::acquire(&app_id, || {}).expect("first acquire should succeed");
    assert!(matches!(primary, SingleInstance::Primary(_)));

    let secondary =
        SingleInstance::acquire(&app_id, || {}).expect("second acquire should not error");
    assert!(matches!(secondary, SingleInstance::Secondary));
}

#[test]
fn primary_receives_a_single_message() {
    let app_id = unique_app_id("single-message");
    let woken = Arc::new(AtomicUsize::new(0));
    let woken_writer = Arc::clone(&woken);

    let instance = SingleInstance::acquire(&app_id, move || {
        woken_writer.fetch_add(1, Ordering::SeqCst);
    })
    .expect("acquire");
    let SingleInstance::Primary(listener) = instance else {
        panic!("expected to become the primary instance");
    };

    send_to_primary(&app_id, &message(&["a.mp3"], false)).expect("send to primary");

    let received = listener
        .recv_timeout(Duration::from_secs(2))
        .expect("message should arrive");
    assert_eq!(received.files, vec![PathBuf::from("a.mp3")]);
    assert!(!received.enqueue);
    assert!(woken.load(Ordering::SeqCst) >= 1, "waker should have fired");
}

#[test]
fn messages_sent_close_together_are_batched_into_one() {
    let app_id = unique_app_id("batching");
    let instance = SingleInstance::acquire(&app_id, || {}).expect("acquire");
    let SingleInstance::Primary(listener) = instance else {
        panic!("expected to become the primary instance");
    };

    // Simulates Explorer spawning one secondary process per selected file.
    for (i, enqueue) in [false, true, false].into_iter().enumerate() {
        let file = format!("{i}.mp3");
        send_to_primary(&app_id, &message(&[&file], enqueue)).expect("send to primary");
    }

    let batch = listener
        .recv_timeout(Duration::from_secs(2))
        .expect("batched message should arrive");
    assert_eq!(batch.files.len(), 3, "all three files should be merged");
    assert!(
        batch.enqueue,
        "enqueue should be true if any message set it"
    );

    // The three sends must have collapsed into exactly one batch.
    assert!(
        listener.recv_timeout(Duration::from_millis(300)).is_none(),
        "no second batch should be delivered"
    );
}

#[test]
fn secondary_send_without_primary_fails_after_retrying() {
    let app_id = unique_app_id("no-primary");
    let result = send_to_primary(&app_id, &message(&["a.mp3"], false));
    assert!(result.is_err());
}
