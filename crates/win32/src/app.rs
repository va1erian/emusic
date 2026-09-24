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
use emusic_ui::shell::{Changes, Shell};
use emusic_ui::state::{Command, View};
use emusic_ui::waker::WakerSlot;
use win32ui::prelude::*;
use win32ui::{column, dip, row};

use crate::menu;
use crate::views::music::{ContextAction, MusicView};
use crate::views::navigator::NavigatorView;
use crate::views::placeholder::Placeholder;
use crate::views::status_bar::StatusBarView;
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
    /// Play the Music view row (double-click / Enter).
    PlayRow(usize),
    /// Sort the Music view by a header column.
    SortColumn(usize),
    /// Open the Music view's context menu for a row.
    ContextRow(usize),
    /// Run a Music view context-menu action.
    ContextAction(ContextAction),
    /// Switch the central view (navigator row click).
    Navigate(View),
    /// Close the window and exit.
    Quit,
}

/// The app: the shell plus the window's controls.
pub struct Win32App {
    shell: Shell,
    navigator: NavigatorView,
    central: Placeholder,
    music: MusicView,
    right_panel: Placeholder,
    status: StatusBarView,
    /// The shell's repaint timer, if scheduled, and its interval in ms.
    timer: Option<(TimerId, u32)>,
    /// Panel visibility last applied, so a change triggers a relayout.
    applied_panels: emusic_ui::state::PanelVisibility,
    /// Theme last applied to the window, so a change re-themes it.
    applied_theme: emusic_ui::state::Theme,
    /// View last applied to the central area, so a change re-lays it out.
    applied_view: View,
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
        let navigator = NavigatorView::new(ui).expect("create navigator view");
        let central = Placeholder::new(ui, "Music").expect("create central placeholder");
        let music = MusicView::new(ui).expect("create music view");
        let right_panel = Placeholder::new(ui, "Now playing").expect("create right panel");
        let status = StatusBarView::new(ui).expect("create status bar");

        waker.bind(Win32Waker::new(ui.proxy()));
        let mut shell = Shell::new(library, player, config, config_path, waker);
        if let Some(ipc) = ipc {
            shell.attach_ipc(ipc);
        }
        if let Some(message) = startup {
            shell.handle_ipc_message(message);
        }

        ui.set_menu_bar(menu::build());
        // The central area shows the Music list for the Music view and the
        // placeholder otherwise; hidden items take no space in the layout.
        let music_active = shell.state.view == View::Music;
        central.set_visible(!music_active);
        music.set_visible(music_active);
        ui.set_layout(column![
            row![
                navigator.width(dip(220.0)),
                central.fill(1),
                music.fill(1),
                right_panel.width(dip(280.0)),
            ]
            .fill(1),
            status,
        ]);
        ui.on_timer(|_| Some(Msg::Timer));

        let applied_panels = shell.state.panels;
        let applied_theme = shell.state.theme;
        let applied_view = shell.state.view;
        let mut app = Self {
            shell,
            navigator,
            central,
            music,
            right_panel,
            status,
            timer: None,
            applied_panels,
            applied_theme,
            applied_view,
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
        self.sync_views(ui, tick.changes);
        self.schedule(ui, tick.next_wake);
    }

    /// Pushes the current shell state into the views.
    fn sync_views(&mut self, ui: &mut Ui<Msg>, changes: Changes) {
        // Central-area routing: the Music view owns the track list, every other
        // view is still a placeholder.
        let view = self.shell.state.view;
        if view != self.applied_view {
            let music = view == View::Music;
            self.central.set_visible(!music);
            self.music.set_visible(music);
            ui.relayout();
            self.applied_view = view;
        }

        self.central
            .sync(&format!("{} view", self.shell.state.view.label()));

        let playing_id = playing_id(self.shell.library.as_ref(), self.shell.player.as_ref());
        self.music.sync(
            &self.shell.state,
            self.shell.library.as_ref(),
            &self.shell.search,
            playing_id,
            changes,
        );

        self.right_panel.sync("Now playing");

        self.shell.state.navigator.sync(self.shell.state.view);
        self.navigator.sync(&self.shell.state.navigator);

        let library = self.shell.library.as_ref();
        let player = self.shell.player.as_ref();
        self.shell
            .state
            .status_bar
            .sync(self.shell.state.search_result_count, library, player);
        self.status
            .sync(&self.shell.state.status_bar, self.shell.backend_notice());

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
            Msg::PlayRow(row) => {
                if let Some(command) = self.music.activate(row) {
                    self.shell.dispatch(command);
                    self.tick(ui);
                }
            }
            Msg::SortColumn(column) => {
                if let Some(id) = crate::views::music::column_id(column) {
                    self.shell.state.music.table.sort.toggle(id);
                    self.music.resort(
                        &self.shell.state,
                        self.shell.library.as_ref(),
                        &self.shell.search,
                    );
                    self.tick(ui);
                }
            }
            Msg::ContextRow(row) => {
                self.music.set_context_row(row);
                ui.popup(self.music.context_menu(), ui.cursor_position());
            }
            Msg::ContextAction(action) => {
                if let Some(command) = self.music.run_context(action, ui.hwnd()) {
                    self.shell.dispatch(command);
                    self.tick(ui);
                }
            }
            Msg::Navigate(view) => {
                self.shell.dispatch(Command::SetView(view));
                self.tick(ui);
            }
            Msg::Quit => ui.close(),
        }
    }
}

/// The library id of the player's current track, matched by path.
fn playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library
        .track_by_path(&now_playing.path)
        .map(|track| track.id)
}

/// Maps the shell's dark/light theme onto win32ui's palette.
fn win32_theme(theme: emusic_ui::state::Theme) -> win32ui::Theme {
    match theme {
        emusic_ui::state::Theme::Dark => win32ui::Theme::dark(),
        emusic_ui::state::Theme::Light => win32ui::Theme::light(),
    }
}
