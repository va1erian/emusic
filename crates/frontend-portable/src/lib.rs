#![forbid(unsafe_code)]

//! Placeholder for emusic's portable frontend.
//!
//! The binary selects a frontend at build time: the native Win32 renderer
//! (`emusic-frontend-win32`) on Windows, and this crate elsewhere. The portable
//! renderer is filled in when emusic's migration to the portable `xui_core`
//! widget layer lands (branch `migration/xui`); the two frontends drive the same
//! [`emusic_ui::shell::Shell`] through [`emusic_ui::startup::Startup`], so
//! adopting it later needs no change in the binary.
//!
//! Until then [`run`] fails loudly rather than starting a half-migrated UI.

use emusic_ui::startup::Startup;

/// The portable entry point — not implemented yet.
pub fn run(_startup: Startup) -> anyhow::Result<()> {
    anyhow::bail!(
        "emusic's portable frontend is not implemented yet; \
         build the native Win32 frontend instead"
    )
}
