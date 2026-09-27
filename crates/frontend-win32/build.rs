//! Build script for the native frontend's test binaries: embeds `emusic.rc`
//! (Common Controls v6 / per-monitor-v2 DPI manifest) so tests that create
//! native controls get the same activation context as `emusic.exe`. The
//! shipped binaries' resources are compiled by `crates/app/build.rs`.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=emusic.rc");
    println!("cargo:rerun-if-changed=emusic.manifest");
    println!("cargo:rerun-if-changed=../../assets/ico/emusic-app.ico");

    embed_resource::compile_for_tests("emusic.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("compile the native frontend test resources");
}
