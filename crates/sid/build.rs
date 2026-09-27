//! Compiles the vendored cRSID engine plus the small Rust-facing shim.
//!
//! Upstream cRSID uses GCC nested functions (a GCC extension that MSVC and
//! clang do not implement), so the C side must be built with a GCC-compatible
//! compiler even though `cc` would otherwise default to the platform compiler:
//! MinGW-w64 GCC on Windows and Homebrew GCC on macOS. On Windows the
//! resulting object files link into the MSVC binary without any MinGW runtime
//! DLLs. [`locate_gcc`] finds a suitable compiler; see the crate's
//! `vendor/README.md`.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/shim.c");
    println!("cargo:rerun-if-changed=vendor/crsid");
    println!("cargo:rerun-if-env-changed=EMUSIC_SID_CC");
    println!("cargo:rerun-if-env-changed=CC");

    let mut build = cc::Build::new();
    build
        .file("src/shim.c")
        .include("vendor/crsid")
        .include("vendor/crsid/C64")
        .include("vendor/crsid/host")
        .std("c11")
        .warnings(false);

    // Override `cc`'s default compiler only where its default cannot build
    // cRSID's nested functions (Windows/MSVC and macOS/Apple clang).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if let Some(gcc) = locate_gcc(&target_os) {
        if target_os == "windows" {
            // MinGW's `gcc`/`ar` need their runtime DLLs on PATH to run.
            if let Some(dir) = gcc.parent() {
                let mut paths = vec![dir.to_path_buf()];
                paths.extend(std::env::split_paths(
                    &std::env::var_os("PATH").unwrap_or_default(),
                ));
                if let Ok(path) = std::env::join_paths(paths) {
                    build.env("PATH", path);
                }
            }
        }
        build.compiler(&gcc);
    }

    build.compile("emusic_crsid");
}

/// A GCC-compatible compiler for the target, or `None` to let `cc` use its
/// default (fine for targets whose default compiler is GNU-compatible).
///
/// `EMUSIC_SID_CC` (or `CC`, when it names a GNU GCC) overrides the per-target
/// search; this is how a non-standard install can be pointed at.
fn locate_gcc(target_os: &str) -> Option<PathBuf> {
    if let Some(gcc) = overridden_cc() {
        return Some(gcc);
    }
    match target_os {
        "windows" => find_mingw_gcc(),
        "macos" => find_homebrew_gcc(),
        _ => None,
    }
}

/// The `EMUSIC_SID_CC`/`CC` override, when it points at a real GNU GCC.
fn overridden_cc() -> Option<PathBuf> {
    for var in ["EMUSIC_SID_CC", "CC"] {
        if let Some(value) = std::env::var_os(var) {
            let path = PathBuf::from(value);
            if path.is_file() && is_gcc(&path) {
                return Some(path);
            }
        }
    }
    None
}

/// Finds MinGW-w64 GCC, preferring the usual install locations then `PATH`.
fn find_mingw_gcc() -> Option<PathBuf> {
    const NAMES: [&str; 2] = ["gcc.exe", "x86_64-w64-mingw32-gcc.exe"];
    for dir in mingw_candidate_dirs() {
        if let Some(path) = first_gcc(&dir, &NAMES) {
            return Some(path);
        }
    }
    search_path(&NAMES)
}

/// Finds Homebrew GCC on macOS (Apple's `/usr/bin/gcc` is a clang shim and is
/// rejected by [`is_gcc`]).
fn find_homebrew_gcc() -> Option<PathBuf> {
    // Versioned names first: Homebrew never wires an unversioned `gcc` to its
    // GNU compiler, so `gcc` here is only ever a user's deliberate symlink.
    const NAMES: [&str; 6] = ["gcc-15", "gcc-14", "gcc-13", "gcc-12", "gcc-11", "gcc"];
    for dir in [Path::new("/opt/homebrew/bin"), Path::new("/usr/local/bin")] {
        if let Some(path) = first_gcc(dir, &NAMES) {
            return Some(path);
        }
    }
    search_path(&NAMES)
}

/// The first existing GCC in `dir` among `names`.
fn first_gcc(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| dir.join(name))
        .find(|candidate| candidate.is_file() && is_gcc(candidate))
}

/// The first GCC among `names` found on `PATH`.
fn search_path(names: &[&str]) -> Option<PathBuf> {
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        if let Some(path) = first_gcc(&dir, names) {
            return Some(path);
        }
    }
    None
}

/// Whether `path`'s file name looks like a GCC executable (not `cl.exe` or a
/// clang-based `gcc` shim) and it actually reports itself as GNU GCC. The name
/// check is cheap; the version probe rejects Apple's `/usr/bin/gcc`, which is
/// clang and cannot compile cRSID's nested functions.
fn is_gcc(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let named = stem == "gcc"
        || stem.ends_with("-gcc")
        || stem
            .strip_prefix("gcc-")
            .is_some_and(|version| version.chars().all(|c| c.is_ascii_digit()));
    named && is_gnu_gcc(path)
}

/// Whether `path`'s `--version` output identifies it as GNU GCC rather than
/// clang (including Apple clang).
fn is_gnu_gcc(path: &Path) -> bool {
    std::process::Command::new(path)
        .arg("--version")
        .output()
        .is_ok_and(|output| {
            let text = String::from_utf8_lossy(&output.stdout);
            text.contains("gcc") && !text.contains("clang") && !text.contains("Apple")
        })
}

/// Common MinGW-w64 / MSYS2 install roots, covering GitHub's `windows-latest`
/// runner (`C:\mingw64`) and typical local MSYS2 installs.
fn mingw_candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for root in [
        r"C:\mingw64",
        r"C:\msys64",
        r"C:\msys32",
        r"E:\msys2",
        r"E:\msys64",
        r"C:\tools\mingw64",
        r"C:\ProgramData\mingw64\mingw64",
        r"C:\ProgramData\chocolatey\lib\mingw\tools\install\mingw64",
    ] {
        let root = PathBuf::from(root);
        for sub in ["bin", "ucrt64/bin", "mingw64/bin"] {
            dirs.push(root.join(sub));
        }
    }
    dirs
}
