#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

//! emusic's thin entry point (#106, #merge): it owns the CLI, the file
//! associations and the single-instance handshake, then hands a
//! [`Startup`](emusic_ui::startup::Startup) to whichever frontend is compiled
//! in — the native Win32 one on Windows, the portable one elsewhere.
//!
//! No toolkit is named here (except `winshell` for the Windows process
//! bootstrap): the frontend crate owns the window and the message loop.

use std::env;

use clap::Parser;
use emusic_ui::cli::Cli;
use emusic_ui::config::{self, Config};
use emusic_ui::startup::{RunFn, Startup};
use emusic_ui::waker::WakerSlot;

// Frontend selection is exhaustive by construction: the native renderer on
// Windows (unless the `portable` feature opts into the portable one) and the
// portable renderer everywhere else. The `FRONTEND` constant below is a
// compile-time check that whichever was selected matches the contract.
#[cfg(any(not(windows), feature = "portable"))]
use emusic_frontend_portable as frontend;
#[cfg(all(windows, not(feature = "portable")))]
use emusic_frontend_win32 as frontend;

/// The selected frontend's entry point, checked against the shared contract.
const FRONTEND: RunFn = frontend::run;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    if cli.register_associations {
        return register_associations();
    }
    if cli.unregister {
        return unregister_associations();
    }

    run(cli)
}

/// Windows bootstrap: the single-instance handshake, then the native frontend.
#[cfg(windows)]
fn run(cli: Cli) -> anyhow::Result<()> {
    use emusic_ui::backend::ipc;
    use emusic_ui::waker::Waker as _;
    use winshell::{IpcMessage, SingleInstance};

    let message = IpcMessage {
        enqueue: cli.enqueue,
        files: cli.files.clone(),
        cwd: env::current_dir().unwrap_or_default(),
    };

    let waker = WakerSlot::new();
    let app_id = ipc::app_id(cli.mock);
    let wake = waker.handle();
    match SingleInstance::acquire(&app_id, move || wake.wake())? {
        SingleInstance::Secondary => {
            if !message.files.is_empty() {
                winshell::instance::send_to_primary(&app_id, &message)?;
            }
            tracing::info!("emusic is already running; forwarded arguments and exiting");
            Ok(())
        }
        SingleInstance::Primary(listener) => {
            let ipc = ipc::IpcBridge::primary(listener);
            FRONTEND(build_startup(cli, Some(ipc), waker))
        }
    }
}

/// Non-Windows bootstrap: no single-instance/IPC pipe yet, so it runs the
/// portable frontend directly.
#[cfg(not(windows))]
fn run(cli: Cli) -> anyhow::Result<()> {
    FRONTEND(build_startup(cli, None, WakerSlot::new()))
}

/// Resolves the configuration and packages the session for the frontend.
fn build_startup(
    cli: Cli,
    ipc: Option<emusic_ui::backend::ipc::IpcBridge>,
    waker: WakerSlot,
) -> Startup {
    let mock = cli.mock;
    // A `--mock` run never touches the real user's config (#135).
    let config_path = if mock { None } else { config::config_path() };
    let config = config_path
        .as_deref()
        .map_or_else(Config::default, config::load);
    emusic_ui::startup::Startup {
        config,
        config_path,
        ipc,
        files: cli.files.clone(),
        enqueue: cli.enqueue,
        waker,
        mock,
    }
}

#[cfg(windows)]
fn register_associations() -> anyhow::Result<()> {
    let exe = env::current_exe()?;
    let manager = winshell::assoc::AssocManager::new("emusic");
    manager.register(&exe, winshell::assoc::EXTENSIONS)?;
    tracing::info!(exe = %exe.display(), "registered emusic file associations");
    Ok(())
}

#[cfg(not(windows))]
fn register_associations() -> anyhow::Result<()> {
    anyhow::bail!("registering file associations is a Windows feature")
}

#[cfg(windows)]
fn unregister_associations() -> anyhow::Result<()> {
    let manager = winshell::assoc::AssocManager::new("emusic");
    manager.unregister()?;
    tracing::info!("removed emusic file associations");
    Ok(())
}

#[cfg(not(windows))]
fn unregister_associations() -> anyhow::Result<()> {
    anyhow::bail!("file associations are a Windows feature")
}
