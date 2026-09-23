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
use emusic::backend::{self, ipc, smtc, thumbbar};
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
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("emusic")
        .with_inner_size([1200.0, 760.0]);
    if let Some(icon) = emusic::window_icon::window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    // Install the taskbar thumbnail-toolbar message hook (#42) before winit
    // runs its loop. It claims button presses and, crucially, wakes egui so
    // the click is drained even while the app is otherwise idle (there is no
    // continuous repaint when paused). The hook runs before the window/egui
    // context exist, so it goes through the same late-bound repaint handle as
    // IPC (#11).
    #[cfg(target_os = "windows")]
    {
        let repaint_hook = repaint.clone();
        options.event_loop_builder = Some(Box::new(move |builder| {
            use winit::platform::windows::EventLoopBuilderExtWindows as _;

            // Register the shell's `TaskbarButtonCreated` message before the
            // window exists, so the hook below recognises it even if the
            // taskbar announces the button while the window is being created.
            // The buttons are added by `ThumbBar::sync` once the hook wakes
            // the app.
            winshell::thumbbar::taskbar_button_created_message();
            let wake = repaint_hook.waker();
            builder.with_msg_hook(move |msg| {
                let claimed = winshell::thumbbar::msg_hook(msg);
                if claimed {
                    wake();
                }
                claimed
            });
        }));
    }

    eframe::run_native(
        "emusic",
        options,
        Box::new(move |cc| {
            repaint.bind(cc.egui_ctx.clone());
            let backends = backend::build(cli.mock);
            let mut app = App::for_run(cc, backends.library, backends.player, cli.mock);
            if let Some(notice) = backends.notice {
                app.set_backend_notice(notice);
            }
            app.attach_smtc(smtc::Smtc::new(window_handle(cc)));
            app.attach_thumbbar(thumbbar::ThumbBar::new(window_handle(cc)));
            app.attach_ipc(ipc::IpcBridge::primary(listener));
            if !startup_message.files.is_empty() {
                app.handle_ipc_message(startup_message);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|err| anyhow::anyhow!("eframe: {err}"))
}

/// Native window handle the OS integrations (SMTC, taskbar buttons) bind to
/// on Windows; `None` elsewhere. Extracted through `raw-window-handle`, the
/// same abstraction eframe uses, so no unsafe pointer juggling is needed here.
#[cfg(target_os = "windows")]
fn window_handle(cc: &eframe::CreationContext<'_>) -> Option<*mut std::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};

    let handle = cc.window_handle().ok()?;
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return None;
    };
    Some(win32.hwnd.get() as *mut std::ffi::c_void)
}

#[cfg(not(target_os = "windows"))]
fn window_handle(_cc: &eframe::CreationContext<'_>) -> Option<*mut std::ffi::c_void> {
    None
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
