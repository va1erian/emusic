//! Integration tests for file association registration.
//!
//! Every test writes under its own throwaway registry namespace
//! (`HKCU\Software\<unique>\...`) via [`AssocRoots::under_namespace`], and
//! always cleans up afterwards — real associations under
//! `HKCU\Software\Classes` are never touched.

use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use winshell::assoc::{AssocManager, AssocRoots, EXTENSIONS};

fn unique_namespace(case: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("winshell-atest-{case}-{}-{nanos}", std::process::id())
}

/// Runs `f` against a manager scoped to a fresh throwaway namespace, then
/// unconditionally wipes that entire namespace (`HKCU\Software\<namespace>`),
/// even if `f` panics.
///
/// `AssocManager::unregister` only removes what production registration
/// writes (it deliberately leaves shared parents like `Classes` and
/// `RegisteredApplications` alone, since those are real, shared registry
/// locations in production). Tests additionally delete the whole per-test
/// namespace directly so nothing throwaway is left behind.
fn with_manager(case: &str, f: impl FnOnce(&AssocManager)) {
    let namespace = unique_namespace(case);
    let manager =
        AssocManager::with_roots("emusic", AssocRoots::under_namespace(&namespace, "emusic"));

    let result = panic::catch_unwind(AssertUnwindSafe(|| f(&manager)));
    let _ = manager.unregister();
    remove_namespace(&namespace);
    if let Err(payload) = result {
        panic::resume_unwind(payload);
    }
}

/// Deletes `HKCU\Software\<namespace>` recursively via `reg.exe`, ignoring
/// failures (e.g. the key never having been created).
fn remove_namespace(namespace: &str) {
    let _ = std::process::Command::new("reg")
        .args(["delete", &format!(r"HKCU\Software\{namespace}"), "/f"])
        .output();
}

#[test]
fn register_then_unregister_round_trips() {
    with_manager("round-trip", |manager| {
        let exe = PathBuf::from(r"C:\Program Files\emusic\emusic.exe");
        manager.register(&exe, &["mp3", "flac"]).expect("register");

        assert!(manager.is_registered("mp3"));
        assert!(manager.is_registered("flac"));
        assert!(!manager.is_registered("wav"), "wav was never registered");
    });
}

#[test]
fn unregister_removes_everything_registered() {
    with_manager("unregister", |manager| {
        let exe = PathBuf::from(r"C:\emusic\emusic.exe");
        manager.register(&exe, &["mp3", "flac"]).expect("register");
        manager.unregister().expect("unregister");

        assert!(!manager.is_registered("mp3"));
        assert!(!manager.is_registered("flac"));
    });
}

#[test]
fn unregister_is_idempotent_without_prior_registration() {
    with_manager("idempotent", |manager| {
        // No `register` call: unregistering an untouched namespace must not
        // error even though none of the keys exist yet.
        manager
            .unregister()
            .expect("unregister without registering");
        assert!(!manager.is_registered("mp3"));
    });
}

#[test]
fn full_extension_list_registers_and_is_queryable() {
    with_manager("full-list", |manager| {
        let exe = PathBuf::from(r"C:\emusic\emusic.exe");
        manager
            .register(&exe, EXTENSIONS)
            .expect("register all extensions");

        for ext in EXTENSIONS {
            assert!(manager.is_registered(ext), "{ext} should be registered");
        }
    });
}
