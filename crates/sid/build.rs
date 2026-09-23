//! Compiles the vendored cRSID engine plus the small Rust-facing shim.
//!
//! Upstream cRSID uses GCC nested functions (a GCC extension that MSVC and
//! clang do not implement), so the C side must be built with a GCC-compatible
//! MinGW-w64 toolchain even though the Rust side targets MSVC. The resulting
//! object files link into the MSVC binary without any MinGW runtime DLLs.
//! [`find_gcc`] locates a suitable compiler; see the crate's `vendor/README.md`.

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

    // On Windows/MSVC the `cc` crate would default to `cl.exe`, which cannot
    // compile cRSID; force the MinGW GCC we found instead.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && let Some(gcc) = find_gcc()
    {
        if let Some(dir) = gcc.parent() {
            // MinGW's `gcc`/`ar` need their runtime DLLs on PATH to run.
            let mut paths = vec![dir.to_path_buf()];
            paths.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            if let Ok(path) = std::env::join_paths(paths) {
                build.env("PATH", path);
            }
        }
        build.compiler(&gcc);
    }

    build.compile("emusic_crsid");
}

/// Finds a GCC-compatible C compiler, preferring an explicit override, then
/// the usual MinGW-w64 install locations, then `gcc` on `PATH`.
///
/// `EMUSIC_SID_CC` (or `CC`, when it names a GCC) overrides the search; this is
/// how a non-standard install can be pointed at.
fn find_gcc() -> Option<PathBuf> {
    for var in ["EMUSIC_SID_CC", "CC"] {
        if let Some(value) = std::env::var_os(var) {
            let path = PathBuf::from(value);
            if is_gcc(&path) && path.is_file() {
                return Some(path);
            }
        }
    }

    for dir in candidate_dirs() {
        for name in ["gcc.exe", "x86_64-w64-mingw32-gcc.exe"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let candidate = dir.join("gcc.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

/// Whether `path`'s file name looks like a GCC executable (not `cl.exe` or
/// `clang.exe`, which cannot compile cRSID's nested functions).
fn is_gcc(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == "gcc" || stem.ends_with("-gcc"))
}

/// Common MinGW-w64 / MSYS2 install roots, covering GitHub's `windows-latest`
/// runner (`C:\mingw64`) and typical local MSYS2 installs.
fn candidate_dirs() -> Vec<PathBuf> {
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
