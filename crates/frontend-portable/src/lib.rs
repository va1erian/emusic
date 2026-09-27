#![forbid(unsafe_code)]

//! Placeholder for emusic's portable frontend.
//!
//! The binary selects a frontend at build time: the native Win32 renderer
//! (`emusic-frontend-win32`) on Windows by default, and this crate on Windows
//! when the `emusic/portable` feature is on and everywhere else. A frontend is
//! any crate exposing `pub fn run(Startup) -> anyhow::Result<()>` — see
//! [`RunFn`](emusic_ui::startup::RunFn) — over the shared
//! [`Shell`](emusic_ui::shell::Shell).
//!
//! This crate is intentionally a **dummy plug**: the real `xui_core` renderer
//! lands with the rest of emusic's migration to the portable widget layer
//! (branch `migration/xui`). Until then [`run`] fails loudly rather than
//! starting a half-migrated UI. Filling it in needs no change in the binary,
//! which already selects it on non-Windows targets.

use emusic_ui::startup::Startup;

/// The portable entry point — not implemented yet.
///
/// Returns a clear error so a non-Windows build (or a Windows build with the
/// `emusic/portable` feature) fails fast instead of showing an empty window.
pub fn run(_startup: Startup) -> anyhow::Result<()> {
    anyhow::bail!(
        "emusic's portable frontend is not implemented yet; \
         build the native Win32 frontend or drop the `emusic/portable` feature"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The placeholder must refuse to run rather than pretend to work.
    #[test]
    fn run_reports_unimplemented() {
        let startup = Startup {
            config: emusic_ui::config::Config::default(),
            config_path: None,
            ipc: None,
            files: Vec::new(),
            enqueue: false,
            waker: emusic_ui::waker::WakerSlot::new(),
            mock: true,
        };
        assert!(run(startup).is_err());
    }
}
