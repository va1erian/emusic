//! Build script (#125): compiles `emusic.rc` — which references the committed
//! `assets/ico/emusic-app.ico` — into the `emusic` binary so Explorer, the
//! taskbar and Alt-Tab show the app icon. Uses `embed-resource` (preferred
//! over `winres`: actively maintained and a better cross-compile story).
//!
//! Only the `emusic` binary gets the resource, not the dev-only `emusic-shot`
//! screenshot tool. On non-Windows targets this is a no-op.

/// Short git revision of the checkout being built, or `unknown` outside git.
fn git_revision() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short=9", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|rev| rev.trim().to_owned())
        .filter(|rev| !rev.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn main() {
    println!("cargo:rustc-env=EMUSIC_GIT_REV={}", git_revision());
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=emusic.rc");
    println!("cargo:rerun-if-changed=../../assets/ico/emusic-app.ico");

    embed_resource::compile_for("emusic.rc", ["emusic"], embed_resource::NONE)
        .manifest_optional()
        .expect("compile the emusic.exe icon resource");
}
