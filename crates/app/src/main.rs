#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

//! A thin shell around `emusic-ui`: `main` handles the CLI, file associations
//! and the single-instance handshake, then opens a `win32ui` window whose app
//! owns the shared [`Shell`](emusic_ui::shell::Shell).

use std::env;

use clap::Parser;
use emusic_ui::backend::{self, ipc};
use emusic_ui::cli::Cli;
use emusic_ui::config::{self, Config};
use emusic_ui::waker::{Waker as _, WakerSlot};
use winshell::{IpcMessage, SingleInstance};

use emusic::app::{Msg, Win32App};
use emusic::theme::win32_theme;
use emusic::window::window_spec;

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

    // The window chrome follows the theme and accent the shell was configured
    // with, so the accent tint (#355) starts from the user's colour.
    let window_theme = win32_theme(config.theme, config.accent);

    // Register the shell's `TaskbarButtonCreated` message before the window
    // exists, so the raw-message hook below recognises it even if the taskbar
    // announces the button while the window is being created (#321).
    #[cfg(target_os = "windows")]
    winshell::thumbbar::taskbar_button_created_message();

    win32ui::run_app(
        window_spec(
            1100.0,
            720.0,
            window_theme,
            config.accent_tint,
            config.accent_tint_strength,
        ),
        move |ui| {
            let backends = backend::build(mock, waker.handle());
            let emusic_ui::backend::Backends {
                library,
                player,
                notice,
            } = backends;
            // Keep a handle on the waker across `Win32App::new` (which binds it)
            // so the taskbar message hook can wake the app even while it is idle.
            let hook_waker = waker.handle();
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
            if mock {
                // A `--mock` run has no preset install layout; serve the same
                // deterministic placeholder list the screenshot tool uses (#338).
                app.seed_placeholder_presets();
            }
            attach_shell_integrations(ui, &mut app, hook_waker);
            app
        },
    )
    .map_err(|err| anyhow::anyhow!("win32ui: {err}"))
}

/// Binds the OS integrations to this window (#320, #321, #322) and installs
/// the taskbar message hook they need.
///
/// The hook forwards the raw message to the winshell taskbar hooks
/// ([`winshell::thumbbar::msg_hook`] and [`winshell::taskbar::msg_hook`]) and
/// wakes the app when either claims one, so an otherwise-idle app still drains
/// the press or renders the requested thumbnail. It has to be installed after
/// [`Win32App::new`] bound the waker but before the window is first shown (the
/// shell announces the taskbar button only after that).
fn attach_shell_integrations(
    ui: &mut win32ui::Ui<Msg>,
    app: &mut Win32App,
    hook_waker: emusic_ui::waker::WakerHandle,
) {
    let hwnd = ui.hwnd().raw();
    app.attach_smtc(emusic::backend::smtc::Smtc::new(Some(
        hwnd as *mut std::ffi::c_void,
    )));
    app.attach_thumbbar(emusic::backend::thumbbar::ThumbBar::new(Some(
        hwnd as isize,
    )));
    app.attach_taskbar_preview(emusic::backend::taskbar::TaskbarPreview::new(
        Some(hwnd as isize),
        hook_waker.clone(),
    ));
    ui.on_raw_message(move |msg| {
        // `|` (not `||`): both hooks must see every message, and either may
        // claim it. The thumbnail-toolbar hook owns `WM_COMMAND`/
        // `TaskbarButtonCreated`; the taskbar hook owns the DWM thumbnail
        // request.
        let claimed = winshell::thumbbar::msg_hook(msg) | winshell::taskbar::msg_hook(msg);
        if claimed {
            hook_waker.wake();
        }
        claimed
    });
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
