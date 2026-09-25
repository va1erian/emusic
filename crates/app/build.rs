//! Build script for the app: compiles `emusic.rc` (the app icon plus the
//! Common Controls v6 / per-monitor-v2 DPI manifest) into `emusic.exe` and
//! `emusic-shot.exe`, and into the test binaries so the smoke test can create
//! controls. On non-Windows targets this is a no-op.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=emusic.rc");
    println!("cargo:rerun-if-changed=emusic.manifest");
    println!("cargo:rerun-if-changed=../../assets/ico/emusic-app.ico");

    // The manifest must reach every binary that creates controls, including the
    // `shot`-gated screenshot tool (#118).
    embed_resource::compile_for("emusic.rc", ["emusic", "emusic-shot"], embed_resource::NONE)
        .manifest_optional()
        .expect("compile the emusic.exe resources");
    embed_resource::compile_for_tests("emusic.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("compile the emusic test resources");
}
