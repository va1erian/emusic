#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

//! A thin shell around `emusic-ui`: `main` handles the CLI, file associations
//! and the single-instance handshake, then runs the portable [`Win32App`] on a
//! chosen backend.
//!
//! The backend is named only here: the Windows backend by default, or the
//! software/canvas backend when `XUI_BACKEND=canvas` and the `canvas` feature
//! is compiled in. Everything the app itself does is portable.

use std::path::PathBuf;
use std::rc::Rc;

use clap::Parser;
use emusic_ui::backend::ipc::IpcBridge;
use emusic_ui::cli::Cli;
use emusic_ui::config::{self, Config};
use emusic_ui::waker::WakerSlot;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::{Backend, PlatformSpec};

use emusic::app::{Msg, Win32App};
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

    run(cli)
}

/// The single-instance handshake (#11). Windows-only: it uses `winshell`'s
/// named pipe. A non-Windows build skips straight to the UI (follow-up #377
/// adds a D-Bus/lock-file equivalent).
#[cfg(windows)]
fn run(cli: Cli) -> anyhow::Result<()> {
    use emusic_ui::waker::Waker as _;
    use winshell::{IpcMessage, SingleInstance};

    let message = IpcMessage {
        enqueue: cli.enqueue,
        files: cli.files.clone(),
        cwd: std::env::current_dir().unwrap_or_default(),
    };

    let waker = WakerSlot::new();
    let app_id = emusic_ui::backend::ipc::app_id(cli.mock);
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
            // Register the shell's `TaskbarButtonCreated` message before the
            // window exists, so the thumbnail-toolbar hook recognises it.
            emusic_platform::prepare();
            let ipc = IpcBridge::primary(listener);
            run_ui(cli, Some(ipc), message.files, waker)
        }
    }
}

/// A non-Windows launch has no single-instance/IPC pipe yet; it runs the UI
/// directly.
#[cfg(not(windows))]
fn run(cli: Cli) -> anyhow::Result<()> {
    let startup = cli.files.clone();
    run_ui(cli, None, startup, WakerSlot::new())
}

/// Opens the main window and runs the event loop until it closes.
fn run_ui(
    cli: Cli,
    ipc: Option<IpcBridge>,
    startup: Vec<PathBuf>,
    waker: WakerSlot,
) -> anyhow::Result<()> {
    let mock = cli.mock;
    // A `--mock` run never touches the real user's config (#135).
    let config_path = if mock { None } else { config::config_path() };
    let config = config_path
        .as_deref()
        .map_or_else(Config::default, config::load);
    let spec = window_spec(1100.0, 720.0);

    #[cfg(windows)]
    if !wants_canvas() {
        return run_native(spec, config, config_path, ipc, startup, waker, mock);
    }

    run_canvas(spec, config, config_path, ipc, startup, waker, mock)
}

/// Whether the user asked for the software backend via `XUI_BACKEND=canvas`.
///
/// On a build without the `canvas` feature the request cannot be honoured, so
/// it is ignored with a warning and the native backend is used.
fn wants_canvas() -> bool {
    let requested = std::env::var("XUI_BACKEND").as_deref() == Ok("canvas");
    if requested && !cfg!(feature = "canvas") {
        tracing::warn!(
            "XUI_BACKEND=canvas but this build has no `canvas` feature; using the native backend"
        );
        return false;
    }
    requested
}

/// Builds the app's backends (real or mock) and constructs the shell around
/// them, applying any startup notice.
fn build_app(
    ui: &mut Ui<Msg>,
    config: Config,
    config_path: Option<PathBuf>,
    ipc: Option<IpcBridge>,
    startup: Vec<PathBuf>,
    waker: WakerSlot,
    mock: bool,
) -> Win32App {
    let backends = emusic_ui::backend::build(mock, waker.handle());
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
        ipc,
        startup,
        waker,
    );
    if let Some(notice) = notice {
        app.set_backend_notice(notice);
    }
    app
}

/// Runs on the native Win32 backend, attaching the Windows shell integration
/// with the window's handle.
#[cfg(windows)]
fn run_native(
    spec: PlatformSpec,
    config: Config,
    config_path: Option<PathBuf>,
    ipc: Option<IpcBridge>,
    startup: Vec<PathBuf>,
    waker: WakerSlot,
    mock: bool,
) -> anyhow::Result<()> {
    use emusic_platform::NativeHandle;

    let backend = Rc::new(xui::xui_win32::Win32Backend::new());
    let handle_backend = Rc::clone(&backend);
    let backend: Rc<dyn Backend> = backend;
    xui::xui_core::run_app(backend, spec, move |ui| {
        let handle = handle_backend
            .window_hwnd(ui.window())
            .map(|hwnd| NativeHandle::from_raw(hwnd.raw() as isize));
        let mut app = build_app(ui, config, config_path, ipc, startup, waker, mock);
        app.attach_shell(emusic_platform::shell(handle, ui.proxy()));
        app
    })
    .map_err(|error| anyhow::anyhow!("xui: {error}"))
}

/// Runs on the software/canvas backend, with no native window and so a no-op
/// shell integration.
#[cfg(any(not(windows), feature = "canvas"))]
fn run_canvas(
    spec: PlatformSpec,
    config: Config,
    config_path: Option<PathBuf>,
    ipc: Option<IpcBridge>,
    startup: Vec<PathBuf>,
    waker: WakerSlot,
    mock: bool,
) -> anyhow::Result<()> {
    let backend: Rc<dyn Backend> = Rc::new(xui::xui_canvas::WinitBackend::new());
    xui::xui_core::run_app(backend, spec, move |ui| {
        let mut app = build_app(ui, config, config_path, ipc, startup, waker, mock);
        app.attach_shell(Box::new(emusic_platform::NullShell));
        app
    })
    .map_err(|error| anyhow::anyhow!("xui: {error}"))
}

#[cfg(all(windows, not(feature = "canvas")))]
fn run_canvas(
    _spec: PlatformSpec,
    _config: Config,
    _config_path: Option<PathBuf>,
    _ipc: Option<IpcBridge>,
    _startup: Vec<PathBuf>,
    _waker: WakerSlot,
    _mock: bool,
) -> anyhow::Result<()> {
    anyhow::bail!("this build has no `canvas` feature; rebuild with --features canvas")
}

#[cfg(windows)]
fn register_associations() -> anyhow::Result<()> {
    emusic_platform::register_associations().map_err(anyhow::Error::msg)?;
    tracing::info!("registered emusic file associations");
    Ok(())
}

#[cfg(not(windows))]
fn register_associations() -> anyhow::Result<()> {
    anyhow::bail!("registering file associations is a Windows feature")
}

#[cfg(windows)]
fn unregister_associations() -> anyhow::Result<()> {
    emusic_platform::unregister_associations().map_err(anyhow::Error::msg)?;
    tracing::info!("removed emusic file associations");
    Ok(())
}

#[cfg(not(windows))]
fn unregister_associations() -> anyhow::Result<()> {
    anyhow::bail!("file associations are a Windows feature")
}
