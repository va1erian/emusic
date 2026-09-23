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
/// even if `f` panics. `f` receives the namespace so it can query values
/// written under it.
///
/// `AssocManager::unregister` only removes what production registration
/// writes (it deliberately leaves shared parents like `Classes` and
/// `RegisteredApplications` alone, since those are real, shared registry
/// locations in production). Tests additionally delete the whole per-test
/// namespace directly so nothing throwaway is left behind.
fn with_manager(case: &str, f: impl FnOnce(&str, &AssocManager)) {
    let namespace = unique_namespace(case);
    let manager =
        AssocManager::with_roots("emusic", AssocRoots::under_namespace(&namespace, "emusic"));

    let result = panic::catch_unwind(AssertUnwindSafe(|| f(&namespace, &manager)));
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
    with_manager("round-trip", |_, manager| {
        let exe = PathBuf::from(r"C:\Program Files\emusic\emusic.exe");
        manager.register(&exe, &["mp3", "flac"]).expect("register");

        assert!(manager.is_registered("mp3"));
        assert!(manager.is_registered("flac"));
        assert!(!manager.is_registered("wav"), "wav was never registered");
    });
}

#[test]
fn unregister_removes_everything_registered() {
    with_manager("unregister", |_, manager| {
        let exe = PathBuf::from(r"C:\emusic\emusic.exe");
        manager.register(&exe, &["mp3", "flac"]).expect("register");
        manager.unregister().expect("unregister");

        assert!(!manager.is_registered("mp3"));
        assert!(!manager.is_registered("flac"));
    });
}

#[test]
fn unregister_is_idempotent_without_prior_registration() {
    with_manager("idempotent", |_, manager| {
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
    with_manager("full-list", |_, manager| {
        let exe = PathBuf::from(r"C:\emusic\emusic.exe");
        manager
            .register(&exe, EXTENSIONS)
            .expect("register all extensions");

        for ext in EXTENSIONS {
            assert!(manager.is_registered(ext), "{ext} should be registered");
        }
    });
}

#[test]
fn default_icon_uses_shipped_icons_when_present() {
    with_manager("icons", |namespace, manager| {
        // A throwaway "install dir" holding emusic.exe plus the icons folder
        // the installer ships (#125).
        let dir = std::env::temp_dir().join(unique_namespace("icons-files"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("icons")).expect("create icons dir");
        let exe = dir.join("emusic.exe");
        std::fs::write(&exe, b"").expect("write placeholder exe");
        std::fs::write(dir.join("icons").join("file-mp3.ico"), b"ico").expect("write mp3 icon");
        std::fs::write(dir.join("icons").join("file-audio.ico"), b"ico")
            .expect("write fallback icon");

        manager.register(&exe, &["mp3", "webm"]).expect("register");

        // mp3 has a dedicated icon; webm does not, so it falls back.
        assert!(
            default_icon(namespace, "mp3").ends_with(r"icons\file-mp3.ico"),
            "{}",
            default_icon(namespace, "mp3")
        );
        assert!(
            default_icon(namespace, "webm").ends_with(r"icons\file-audio.ico"),
            "{}",
            default_icon(namespace, "webm")
        );

        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn default_icon_falls_back_to_the_exe_without_an_icons_folder() {
    with_manager("no-icons", |namespace, manager| {
        let exe = PathBuf::from(r"C:\emusic\emusic.exe");
        manager.register(&exe, &["mp3"]).expect("register");

        let icon = default_icon(namespace, "mp3");
        assert!(icon.ends_with(r#"emusic.exe",0"#), "{icon}");
    });
}

#[test]
fn applications_entry_has_friendly_name_and_open_command() {
    with_manager("applications", |namespace, manager| {
        let exe = PathBuf::from(r"C:\emusic\emusic.exe");
        manager.register(&exe, &["mp3"]).expect("register");

        let key = format!(r"HKCU\Software\{namespace}\Classes\Applications\emusic.exe");
        assert_eq!(query_value(&key, "FriendlyAppName"), "emusic");
        assert_eq!(
            query_value(&format!(r"{key}\shell\open\command"), ""),
            r#""C:\emusic\emusic.exe" "%1""#
        );
    });
}

/// Reads a ProgID's `DefaultIcon` value from the throwaway namespace with
/// `reg.exe`, unquoted.
fn default_icon(namespace: &str, ext: &str) -> String {
    let key = format!(r"HKCU\Software\{namespace}\Classes\emusic.{ext}\DefaultIcon");
    query_value(&key, "").trim_matches('"').to_string()
}

/// Reads a registry value (`""` for the key's default value) with `reg.exe`.
fn query_value(key: &str, value_name: &str) -> String {
    let mut args = vec!["query", key];
    if value_name.is_empty() {
        args.push("/ve");
    } else {
        args.extend(["/v", value_name]);
    }
    let output = std::process::Command::new("reg")
        .args(&args)
        .output()
        .expect("run reg query");
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .find_map(|line| {
            line.split_once("REG_SZ")
                .map(|(_, value)| value.trim().to_string())
        })
        .unwrap_or_default()
}
