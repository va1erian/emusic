//! The native Win32 frontend's application object (#106).
//!
//! [`Win32App`] owns the toolkit-agnostic [`Shell`] and the window's controls.
//! `win32ui` drives it: every widget event is mapped to [`Msg`], delivered to
//! [`Win32App::update`], which runs [`Shell::tick`] on wakes and timers, syncs
//! the views, and schedules the next timer from [`Tick::next_wake`].

use std::time::Instant;

use emusic_ui::backend::ipc::IpcBridge;
use emusic_ui::config::Config;
use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::player_api::PlayerApi;
use emusic_ui::shell::Shell;
use emusic_ui::state::Command;
use emusic_ui::waker::WakerSlot;
use win32ui::prelude::*;
use win32ui::{StatusBar, column, dip, row};

use crate::menu;
use crate::views::placeholder::Placeholder;
use crate::waker::Win32Waker;

/// Everything the window can ask the app to do.
pub enum Msg {
    /// A background worker (search, IPC, image decode) woke the UI.
    Wake,
    /// The shell's repaint timer fired.
    Timer,
    /// Apply a command from the menu bar.
    Dispatch(Command),
    /// Open File -> Database info (placeholder until the dialog lands).
    DatabaseInfo,
    /// Close the window and exit.
    Quit,
}

/// The app: the shell plus the window's controls.
pub struct Win32App {
    shell: Shell,
    navigator: Placeholder,
    central: Placeholder,
    right_panel: Placeholder,
    status: StatusBar<Msg>,
    /// The shell's repaint timer, if scheduled, and its interval in ms.
    timer: Option<(TimerId, u32)>,
    /// Panel visibility last applied, so a change triggers a relayout.
    applied_panels: emusic_ui::state::PanelVisibility,
    /// Theme last applied to the window, so a change re-themes it.
    applied_theme: emusic_ui::state::Theme,
}

impl Win32App {
    /// Builds the app: binds the frontend's [`Waker`](emusic_ui::waker::Waker)
    /// to the window, creates the shell and the window's controls, installs
    /// the menu bar and the layout, then ticks once so the views are populated.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ui: &mut Ui<Msg>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
        config: Config,
        config_path: Option<std::path::PathBuf>,
        ipc: Option<IpcBridge>,
        startup: Option<winshell::IpcMessage>,
        waker: WakerSlot,
    ) -> Self {
        let navigator = Placeholder::new(ui, "Navigator").expect("create navigator placeholder");
        let central = Placeholder::new(ui, "Music").expect("create central placeholder");
        let right_panel = Placeholder::new(ui, "Now playing").expect("create right panel");
        let status = StatusBar::new(ui).expect("create status bar");

        waker.bind(Win32Waker::new(ui.proxy()));
        let mut shell = Shell::new(library, player, config, config_path, waker);
        if let Some(ipc) = ipc {
            shell.attach_ipc(ipc);
        }
        if let Some(message) = startup {
            shell.handle_ipc_message(message);
        }

        ui.set_menu_bar(menu::build());
        ui.set_layout(column![
            row![
                navigator.width(dip(220.0)),
                central.fill(1),
                right_panel.width(dip(280.0)),
            ]
            .fill(1),
            status,
        ]);
        ui.on_timer(|_| Some(Msg::Timer));

        let applied_panels = shell.state.panels;
        let applied_theme = shell.state.theme;
        let mut app = Self {
            shell,
            navigator,
            central,
            right_panel,
            status,
            timer: None,
            applied_panels,
            applied_theme,
        };
        app.tick(ui);
        app
    }

    /// Sets the one-line startup notice shown in the status bar.
    pub fn set_backend_notice(&mut self, notice: impl Into<String>) {
        self.shell.set_backend_notice(notice);
    }

    /// Runs the shell for this frame, syncs the views and schedules the timer.
    fn tick(&mut self, ui: &mut Ui<Msg>) {
        let tick = self.shell.tick(Instant::now());
        self.sync_views(ui);
        self.schedule(ui, tick.next_wake);
    }

    /// Pushes the current shell state into the placeholder views.
    fn sync_views(&mut self, ui: &mut Ui<Msg>) {
        let tracks = self.shell.library.track_count();
        self.central.sync(&format!(
            "{} view — {tracks} tracks",
            self.shell.state.view.label()
        ));

        let folders = self.shell.library.folders().len();
        self.navigator
            .sync(&format!("Navigator — {folders} folders"));
        self.right_panel.sync("Now playing");

        let status = self
            .shell
            .backend_notice()
            .map(ToString::to_string)
            .or_else(|| self.shell.library.status_text())
            .unwrap_or_else(|| format!("{} tracks", self.shell.library.track_count()));
        self.status.set_text(0, &status);

        // Panel visibility is toggled through `Command::TogglePanel`; apply it
        // and relayout only when it actually changed.
        let panels = self.shell.state.panels;
        if panels != self.applied_panels {
            self.navigator.set_visible(panels.navigator);
            self.right_panel.set_visible(panels.right_panel);
            self.status.set_visible(panels.status_bar);
            ui.relayout();
            self.applied_panels = panels;
        }

        // The dark/light menu toggle changes the shell's theme; mirror it onto
        // the window (the accent is a follow-up: win32ui's palette is richer).
        let theme = self.shell.state.theme;
        if theme != self.applied_theme {
            ui.set_theme(win32_theme(theme));
            self.applied_theme = theme;
        }
    }

    /// Keeps a single repeating timer in step with the shell's `next_wake`.
    fn schedule(&mut self, ui: &mut Ui<Msg>, next_wake: Option<std::time::Duration>) {
        let wanted = next_wake.map(|wait| wait.as_millis().min(u128::from(u32::MAX)) as u32);
        match wanted {
            Some(ms) if self.timer.is_none_or(|(_, current)| current != ms) => {
                if let Some((id, _)) = self.timer.take() {
                    ui.kill_timer(id);
                }
                if let Ok(id) = ui.set_timer(ms.max(1)) {
                    self.timer = Some((id, ms));
                }
            }
            None => {
                if let Some((id, _)) = self.timer.take() {
                    ui.kill_timer(id);
                }
            }
            Some(_) => {}
        }
    }
}

impl App for Win32App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Wake | Msg::Timer => self.tick(ui),
            Msg::Dispatch(command) => {
                self.shell.dispatch(command);
                self.tick(ui);
            }
            Msg::DatabaseInfo => {
                self.shell.state.database_info_open = true;
            }
            Msg::Quit => ui.close(),
        }
    }
}

/// Maps the shell's dark/light theme onto win32ui's palette.
fn win32_theme(theme: emusic_ui::state::Theme) -> win32ui::Theme {
    match theme {
        emusic_ui::state::Theme::Dark => win32ui::Theme::dark(),
        emusic_ui::state::Theme::Light => win32ui::Theme::light(),
    }
}
