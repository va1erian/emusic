//! The portable entry point: build the `xui_core` window and run
//! [`Win32App`](crate::app::Win32App).
//!
//! The binary owns the CLI, file associations and the single-instance
//! handshake; it hands a [`Startup`] here. This module runs the shared app on
//! `xui`'s software `canvas` backend. The backend is named only here; every
//! view names only portable widgets.

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

/// Opens the main window on the canvas backend and runs until it closes.
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

    // Register the Windows shell hooks before the window exists, so the
    // thumbnail-toolbar hook recognises the taskbar button it creates.
    #[cfg(windows)]
    emusic_platform::prepare();

    let spec = window_spec(1100.0, 720.0);

    run_canvas(spec, config, config_path, ipc, files, waker, mock)
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
        mock,
    );
    if let Some(notice) = notice {
        app.set_backend_notice(notice);
    }
    let chrome = WindowChrome::new(backend, ui.window());
    app.attach_chrome(chrome);
    schedule_smoke_exit(ui);
    app
}

/// If `EMUSIC_SMOKE_EXIT_MS` is set, posts [`Msg::Quit`] after that many
/// milliseconds so CI can run the real shell non-interactively (the macOS
/// port's smoke run). A no-op in normal runs.
fn schedule_smoke_exit(ui: &Ui<Msg>) {
    let Ok(value) = std::env::var("EMUSIC_SMOKE_EXIT_MS") else {
        return;
    };
    let Ok(ms) = value.parse::<u64>() else {
        tracing::warn!(value, "ignoring non-numeric EMUSIC_SMOKE_EXIT_MS");
        return;
    };
    let proxy = ui.proxy();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        let _ = proxy.send(Msg::Quit);
    });
}

/// Runs on the software/canvas backend, with no native window and so a no-op
/// shell integration.
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
        // The canvas backend has no native window handle; the platform picks
        // the right shell (Windows SMTC, macOS Now Playing, or a no-op).
        app.attach_shell(emusic_platform::shell(None, ui.proxy()));
        app
    })
    .map_err(|error| anyhow::anyhow!("xui: {error}"))
}
