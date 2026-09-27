//! The portable entry point: pick a backend and run [`Win32App`] on the
//! `xui_core` runtime.

use std::path::PathBuf;
use std::rc::Rc;

use emusic_ui::backend::ipc::IpcBridge;
use emusic_ui::config::Config;
use emusic_ui::startup::Startup;
use emusic_ui::waker::WakerSlot;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::{Backend, PlatformSpec};

use crate::app::{Msg, Win32App};
use crate::window::{WindowChrome, window_spec};

/// Opens the main window on the backend chosen for this target and runs until
/// it closes.
pub fn run(startup: Startup) -> anyhow::Result<()> {
    let Startup {
        config,
        config_path,
        ipc,
        files,
        enqueue: _,
        waker,
        mock,
    } = startup;
    let spec = window_spec(1100.0, 720.0);

    #[cfg(windows)]
    if !wants_canvas() {
        return run_native(spec, config, config_path, ipc, files, waker, mock);
    }

    run_canvas(spec, config, config_path, ipc, files, waker, mock)
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
/// them, applying any startup notice and the window chrome.
#[allow(clippy::too_many_arguments)]
fn build_app(
    ui: &mut Ui<Msg>,
    backend: Rc<dyn Backend>,
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
    let chrome = WindowChrome::new(backend, ui.window());
    app.attach_chrome(chrome);
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
    let chrome_backend = Rc::clone(&backend);
    xui::xui_core::run_app(backend, spec, move |ui| {
        let handle = handle_backend
            .window_hwnd(ui.window())
            .map(|hwnd| NativeHandle::from_raw(hwnd.raw() as isize));
        let mut app = build_app(
            ui,
            chrome_backend,
            config,
            config_path,
            ipc,
            startup,
            waker,
            mock,
        );
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
    let chrome_backend = Rc::clone(&backend);
    xui::xui_core::run_app(backend, spec, move |ui| {
        let mut app = build_app(
            ui,
            chrome_backend,
            config,
            config_path,
            ipc,
            startup,
            waker,
            mock,
        );
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
