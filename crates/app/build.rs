//! Build script (#125): compiles `emusic.rc` — which references the committed
//! `assets/ico/emusic-app.ico` — into the `emusic` binary so Explorer, the
//! taskbar and Alt-Tab show the app icon. Uses `embed-resource` (preferred
//! over `winres`: actively maintained and a better cross-compile story).
//!
//! Only the `emusic` binary gets the resource, not the dev-only `emusic-shot`
//! screenshot tool. On non-Windows targets this is a no-op.

fn main() {
    println!("cargo:rerun-if-changed=emusic.rc");
    println!("cargo:rerun-if-changed=../../assets/ico/emusic-app.ico");

    embed_resource::compile_for("emusic.rc", ["emusic"], embed_resource::NONE)
        .manifest_optional()
        .expect("compile the emusic.exe icon resource");
}
