//! The native entry point: build the `win32ui` window and run [`Win32App`].

use anyhow::anyhow;

use emusic_ui::startup::Startup;
use emusic_ui::waker::Waker as _;

use crate::app::{Msg, Win32App};
use crate::icon;
use crate::theme::win32_theme;
use crate::window::window_spec;

/// Opens the main window on the native Win32 backend and runs until it closes.
pub fn run(startup: Startup) -> anyhow::Result<()> {
    let Startup {
        config,
        config_path,
        ipc,
        files,
        enqueue,
        waker,
        mock,
    } = startup;

    // The window chrome follows the theme and accent the shell was configured
    // with, so the accent tint (#355) starts from the user's colour.
    let window_theme = win32_theme(config.theme, config.accent);

    // Register the shell's `TaskbarButtonCreated` message before the window
    // exists, so the raw-message hook recognises it even if the taskbar
    // announces the button while the window is being created (#321).
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
            icon::install(ui);
            let backends = emusic_ui::backend::build(mock, waker.handle());
            let emusic_ui::backend::Backends {
                library,
                player,
                notice,
            } = backends;
            // Keep a handle on the waker across `Win32App::new` (which binds it)
            // so the taskbar message hook can wake the app even while it is idle.
            let hook_waker = waker.handle();
            let startup = (!files.is_empty()).then(|| winshell::IpcMessage {
                enqueue,
                files: files.clone(),
                cwd: std::env::current_dir().unwrap_or_default(),
            });
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
            if mock {
                // A `--mock` run has no preset install layout; serve the same
                // deterministic placeholder list the screenshot tool uses (#338).
                app.seed_placeholder_presets();
            }
            attach_shell_integrations(ui, &mut app, hook_waker);
            app
        },
    )
    .map_err(|err| anyhow!("win32ui: {err}"))
}

/// Binds the OS integrations to this window (#320, #321, #322) and installs the
/// taskbar message hook they need.
fn attach_shell_integrations(
    ui: &mut win32ui::Ui<Msg>,
    app: &mut Win32App,
    hook_waker: emusic_ui::waker::WakerHandle,
) {
    let hwnd = ui.hwnd().raw();
    app.attach_smtc(crate::backend::smtc::Smtc::new(Some(
        hwnd as *mut std::ffi::c_void,
    )));
    app.attach_thumbbar(crate::backend::thumbbar::ThumbBar::new(Some(hwnd as isize)));
    app.attach_taskbar_preview(crate::backend::taskbar::TaskbarPreview::new(
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
