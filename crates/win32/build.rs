//! Build script for the native Win32 frontend: compiles `emusic-win32.rc`
//! (the app icon plus the Common Controls v6 / per-monitor-v2 DPI manifest)
//! into `emusic-win32.exe` and `emusic-win32-shot.exe`, and into the test
//! binaries so the smoke test can create controls. On non-Windows targets this
//! is a no-op.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=emusic-win32.rc");
    println!("cargo:rerun-if-changed=emusic-win32.manifest");
    println!("cargo:rerun-if-changed=../../assets/ico/emusic-app.ico");

    // The manifest must reach every binary that creates controls, including the
    // `shot`-gated screenshot tool (#118).
    embed_resource::compile_for(
        "emusic-win32.rc",
        ["emusic-win32", "emusic-win32-shot"],
        embed_resource::NONE,
    )
    .manifest_optional()
    .expect("compile the emusic-win32.exe resources");
    embed_resource::compile_for_tests("emusic-win32.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("compile the emusic-win32 test resources");
}
