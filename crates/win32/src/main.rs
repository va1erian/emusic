#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

//! A thin shell around `emusic-ui`: `main` mirrors the egui binary's flow
//! (CLI, file associations, single instance), then opens a `win32ui` window
//! whose app owns the shared [`Shell`](emusic_ui::shell::Shell).

use std::env;

use clap::Parser;
use emusic_ui::backend::{self, ipc};
use emusic_ui::cli::Cli;
use emusic_ui::config::{self, Config};
use emusic_ui::waker::{Waker as _, WakerSlot};
use win32ui::prelude::*;
use winshell::{IpcMessage, SingleInstance};

use emusic_win32::app::Win32App;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    if cli.register_associations {
        return register_associations();
    }
    if cli.unregister {
        return unregister_associations();
    }

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
        SingleInstance::Primary(listener) => run_ui(cli, message, waker, listener),
    }
}

/// Opens the main window and runs the message loop until it closes.
fn run_ui(
    cli: Cli,
    startup: IpcMessage,
    waker: WakerSlot,
    listener: winshell::Listener,
) -> anyhow::Result<()> {
    let mock = cli.mock;
    // A `--mock` run never touches the real user's config (#135).
    let config_path = if mock { None } else { config::config_path() };
    let config = config_path
        .as_deref()
        .map_or_else(Config::default, config::load);
    let startup = (!startup.files.is_empty()).then_some(startup);
    let ipc = ipc::IpcBridge::primary(listener);

    win32ui::run_app(
        WindowSpec::new("emusic")
            .size(dip(1100.0), dip(720.0))
            .theme(Theme::dark()),
        move |ui| {
            let backends = backend::build(mock);
            let emusic_ui::backend::Backends {
                library,
                player,
                notice,
            } = backends;
            let mut app = Win32App::new(
                ui,
                library,
                player,
                config,
                config_path,
                Some(ipc),
                startup,
                waker,
            );
            if let Some(notice) = notice {
                app.set_backend_notice(notice);
            }
            app
        },
    )
    .map_err(|err| anyhow::anyhow!("win32ui: {err}"))
}

fn register_associations() -> anyhow::Result<()> {
    let exe = env::current_exe()?;
    let manager = winshell::assoc::AssocManager::new("emusic");
    manager.register(&exe, winshell::assoc::EXTENSIONS)?;
    tracing::info!(exe = %exe.display(), "registered emusic file associations");
    Ok(())
}

fn unregister_associations() -> anyhow::Result<()> {
    let manager = winshell::assoc::AssocManager::new("emusic");
    manager.unregister()?;
    tracing::info!("removed emusic file associations");
    Ok(())
}
