//! The portable entry point: build the `xui_core` window and run
//! [`Win32App`](crate::app::Win32App).
//!
//! The binary owns the CLI, file associations and the single-instance
//! handshake; it hands a [`Startup`] here. [`run`] runs the shared app on
//! `xui`'s software `canvas` backend; [`run_on`] runs it on a backend a host
//! brings (LazyOS's compositor), with the host's audio. The backend is named
//! only here; every view names only portable widgets.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use emusic_player::AudioBackend;
use emusic_ui::backend::ipc::IpcBridge;
use emusic_ui::config::Config;
use emusic_ui::startup::Startup;
use emusic_ui::waker::WakerSlot;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::Backend;

use crate::app::{Msg, Win32App};
use crate::window::{WindowChrome, window_spec};

/// The main window's size, in device-independent pixels.
const WINDOW_SIZE: (f32, f32) = (1100.0, 720.0);

/// A window system and sound system to run the app on, for a host that is not
/// one of the canvas backend's platforms.
pub struct Host {
    /// The window backend.
    pub backend: Rc<dyn Backend>,
    /// Whether the window system draws the window's frame and buttons, so the
    /// app leaves out its own caption band.
    pub native_chrome: bool,
    /// The backend's name, for the About page.
    pub label: &'static str,
    /// The window's size in device-independent pixels; the default size when
    /// `None`.
    pub size: Option<(f32, f32)>,
    /// What plays the tracks; BASS when `None`.
    pub audio: Option<Arc<dyn AudioBackend>>,
}

/// Opens the main window on the canvas backend and runs until it closes.
#[cfg(feature = "winit")]
pub fn run(startup: Startup) -> anyhow::Result<()> {
    run_on(
        startup,
        Host {
            backend: Rc::new(xui::xui_canvas::WinitBackend::new()),
            native_chrome: cfg!(target_os = "macos"),
            label: "canvas (software)",
            size: None,
            audio: None,
        },
    )
}

/// Opens the main window on `host`'s backend and runs until it closes.
pub fn run_on(startup: Startup, host: Host) -> anyhow::Result<()> {
    let Startup {
        config,
        config_path,
        ipc,
        files,
        enqueue: _,
        waker,
        mock,
    } = startup;
    let Host {
        backend,
        native_chrome,
        label,
        size,
        audio,
    } = host;
    crate::backend::set_host(native_chrome, label);

    // Register the Windows shell hooks before the window exists, so the
    // thumbnail-toolbar hook recognises the taskbar button it creates.
    #[cfg(windows)]
    emusic_platform::prepare();

    let (width, height) = size.unwrap_or(WINDOW_SIZE);
    let spec = window_spec(width, height);
    let session = Session {
        config,
        config_path,
        ipc,
        startup: files,
        waker,
        mock,
        audio,
    };
    let chrome_backend = Rc::clone(&backend);
    xui::xui_core::run_app(backend, spec, move |ui| {
        let mut app = build_app(ui, chrome_backend, session);
        // The platform picks the right shell (Windows SMTC, macOS Now
        // Playing, or a no-op); the window has no native handle to give it.
        app.attach_shell(emusic_platform::shell(None, ui.proxy()));
        app
    })
    .map_err(|error| anyhow::anyhow!("xui: {error}"))
}

/// Everything the app is built from once the window exists.
struct Session {
    config: Config,
    config_path: Option<PathBuf>,
    ipc: Option<IpcBridge>,
    startup: Vec<PathBuf>,
    waker: WakerSlot,
    mock: bool,
    audio: Option<Arc<dyn AudioBackend>>,
}

/// Builds the app's backends (real or mock) and constructs the shell around
/// them, applying any startup notice and the window chrome.
fn build_app(ui: &mut Ui<Msg>, backend: Rc<dyn Backend>, session: Session) -> Win32App {
    let backends =
        emusic_ui::backend::build_with(session.mock, session.waker.handle(), session.audio);
    let emusic_ui::backend::Backends {
        library,
        player,
        notice,
    } = backends;
    let mut app = Win32App::new(
        ui,
        library,
        player,
        session.config,
        session.config_path,
        session.ipc,
        session.startup,
        session.waker,
        session.mock,
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
