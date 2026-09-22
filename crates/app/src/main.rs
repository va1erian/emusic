#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

//! Thin entry point (#11): parses the CLI, handles the one-shot
//! `--register-associations`/`--unregister` actions, acquires single
//! instance *before* creating any window (forwarding to, and exiting in
//! favor of, an already-running primary), then starts the UI. All the
//! actual wiring lives in [`emusic::backend`] and [`emusic::cli`].

use std::env;

use clap::Parser;
use eframe::egui;
use winshell::{IpcMessage, SingleInstance};

use emusic::app::App;
use emusic::backend::{self, ipc};
use emusic::cli::Cli;

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

    let repaint = ipc::RepaintHandle::new();
    let app_id = ipc::app_id(cli.mock);
    match SingleInstance::acquire(&app_id, repaint.waker())? {
        SingleInstance::Secondary => {
            if !message.files.is_empty() {
                winshell::instance::send_to_primary(&app_id, &message)?;
            }
            tracing::info!("emusic is already running; forwarded arguments and exiting");
            Ok(())
        }
        SingleInstance::Primary(listener) => run_ui(cli, message, repaint, listener),
    }
}

fn run_ui(
    cli: Cli,
    startup_message: IpcMessage,
    repaint: ipc::RepaintHandle,
    listener: winshell::Listener,
) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("emusic")
            .with_inner_size([1200.0, 760.0]),
        ..Default::default()
    };

    eframe::run_native(
        "emusic",
        options,
        Box::new(move |cc| {
            repaint.bind(cc.egui_ctx.clone());
            let backends = backend::build(cli.mock);
            let mut app = App::new(cc, backends.library, backends.player);
            if let Some(notice) = backends.notice {
                app.set_backend_notice(notice);
            }
            app.attach_ipc(ipc::IpcBridge::primary(listener));
            if !startup_message.files.is_empty() {
                app.handle_ipc_message(startup_message);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|err| anyhow::anyhow!("eframe: {err}"))
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
