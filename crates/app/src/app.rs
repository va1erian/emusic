//! The application object (#106), on the portable `xui_core` runtime.
//!
//! [`Win32App`] owns the toolkit-agnostic [`Shell`] and the window's views.
//! `xui_core` drives it: widget events map to [`Msg`], the runtime calls
//! [`Win32App::update`], which runs [`Shell::tick`] on wakes and timers, syncs
//! the views, and schedules the next timer from `Tick::next_wake`.
//!
//! Only the shell layout and the Music view are ported here (#370); the other
//! central views are documented [`Placeholder`]s owned by later issues. OS
//! shell services go through [`emusic_platform`], whose signature names no
//! backend type.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use emusic_platform::{NowPlaying, NullShell, ShellAction, ShellIntegration, ThumbButton};
use emusic_ui::backend::ipc::IpcBridge;
use emusic_ui::config::Config;
use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::player_api::{PlaybackStatus, PlayerApi};
use emusic_ui::shell::{Changes, Shell};
use emusic_ui::state::{Command, ShortcutAction, View, shortcut_command};
use emusic_ui::views::folders::FoldersMsg;
use emusic_ui::views::{Commands, Ctx};
use emusic_ui::waker::WakerSlot;
use xui::xui_core::app::{App, Ui};
use xui::xui_core::backend::{Event, TimerId, WidgetId};
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;

use crate::theme::app_theme;
use crate::views::folders::FoldersView;
use crate::views::music::MusicView;
use crate::views::navigator::NavigatorView;
use crate::views::placeholder::Placeholder;
use crate::views::status_bar::StatusBarView;
use crate::views::top_bar::TopBarView;
use crate::views::track_table::{self, ContextAction};
use crate::waker::UiWaker;
use crate::window::{CAPTION_HEIGHT, WindowChrome};

/// The transport band's height, in device-independent pixels.
const TOP_BAR_HEIGHT: f32 = 40.0;
/// The bottom status band's height, in device-independent pixels.
const STATUS_BAR_HEIGHT: f32 = 24.0;

/// Everything the window can ask the app to do.
pub enum Msg {
    /// A background worker (search, IPC, image decode) woke the UI.
    Wake,
    /// The window's client area changed size.
    Resize,
    /// The shell's repaint timer fired.
    Timer,
    /// Apply a command from the menu bar.
    Dispatch(Command),
    /// Switch the central view (navigator row click).
    Navigate(View),
    /// The Folders tree selected a directory.
    FoldersSelect(String),
    /// The Folders view's "include subfolders" checkbox changed.
    FoldersSubfolders(bool),
    /// Play the Music view row (double-click / Enter).
    PlayRow(usize),
    /// Toggle the star of a track table row.
    ToggleStarRow(usize),
    /// Sort the Music view by a header column.
    SortColumn(usize),
    /// Open the context menu for a Music view row.
    ContextRow(usize),
    /// Run a track-table context-menu action.
    ContextAction(ContextAction),
    /// Start a shuffled playback over the Music view's currently visible
    /// tracks (its header's "Shuffle all" button, #242).
    MusicShuffleAll,
    /// A keyboard shortcut from the central `SHORTCUTS` table fired.
    Shortcut(ShortcutAction),
    /// A transport action the OS shell asked for (#320, #321).
    Shell(ShellAction),
    /// Minimize the window (a portable window button, non-Windows chrome).
    Minimize,
    /// Toggle the window between maximized and restored (non-Windows chrome).
    ToggleMaximize,
    /// Close the window and exit.
    Quit,
}

impl From<ShellAction> for Msg {
    fn from(action: ShellAction) -> Msg {
        Msg::Shell(action)
    }
}

/// The app: the shell plus the window's views.
pub struct Win32App {
    shell: Shell,
    ui: Ui<Msg>,
    navigator: NavigatorView,
    top_bar: TopBarView,
    status_bar: StatusBarView,
    music: MusicView,
    folders: FoldersView,
    placeholder: Placeholder,
    /// The window-level chrome (drag region, window buttons), attached by the
    /// binary once the backend exists; `None` in headless runs.
    chrome: Option<WindowChrome>,
    /// The OS shell integration. A [`NullShell`] until the binary attaches the
    /// real one (headless runs never do).
    shell_integration: Box<dyn ShellIntegration>,
    /// The shell's repaint timer, if scheduled, and its interval in ms.
    timer: Option<(TimerId, u32)>,
    /// View last applied, so a change re-lays the central area out.
    applied_view: View,
    /// Panel visibility last applied, so toggling one re-lays the shell out.
    applied_panels: emusic_ui::state::PanelVisibility,
    /// Theme and accent last applied to the window, so a change re-themes it.
    applied_look: (emusic_ui::state::Theme, emusic_ui::state::Accent),
    /// When the views were last fully synced.
    last_full_sync: Instant,
    /// The row the track context menu was opened on, while a context action is
    /// pending.
    context_row: Option<usize>,
}

impl Win32App {
    /// Builds the app: binds the frontend's [`Waker`](emusic_ui::waker::Waker)
    /// to the window, creates the shell and the window's views, applies the
    /// saved theme and appearance, then ticks once so the views are populated.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ui: &mut Ui<Msg>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
        config: Config,
        config_path: Option<PathBuf>,
        ipc: Option<IpcBridge>,
        startup_files: Vec<PathBuf>,
        waker: WakerSlot,
    ) -> Self {
        let look = (config.theme, config.accent);
        ui.set_theme(app_theme(look.0, look.1));

        let navigator = NavigatorView::new(ui);
        let top_bar = TopBarView::new(ui);
        let status_bar = StatusBarView::new(ui);
        let music = MusicView::new(ui);
        let folders = FoldersView::new(ui);
        let placeholder = Placeholder::new(ui, "Music", "later issues");

        waker.bind(UiWaker::new(ui.proxy()));
        let mut shell = Shell::new(library, player, config, config_path, waker);
        if let Some(ipc) = ipc {
            shell.attach_ipc(ipc);
        }
        if !startup_files.is_empty() {
            shell.player.replace_and_play(&startup_files, 0);
        }

        // The portable runtime has no dedicated resize hook; a window-level
        // event mapper observes `Event::Resize` and relayouts.
        ui.register_events(WidgetId::NONE, |event| match event {
            Event::Resize { .. } => Some(Msg::Resize),
            _ => None,
        });
        ui.on_close(|| Some(Msg::Quit));
        ui.on_timer(|_| Some(Msg::Timer));

        let applied_view = shell.state.view;
        let applied_panels = shell.state.panels;
        let mut app = Win32App {
            shell,
            ui: ui.clone(),
            navigator,
            top_bar,
            status_bar,
            music,
            folders,
            placeholder,
            chrome: None,
            shell_integration: Box::new(NullShell),
            timer: None,
            applied_view,
            applied_panels,
            applied_look: look,
            last_full_sync: Instant::now(),
            context_row: None,
        };
        app.apply_visibility();
        app.relayout();
        app.tick_inner();
        app
    }

    /// Attaches the window chrome, so the top bar can drag the window and the
    /// portable window buttons can act on it. Only the real binary calls this;
    /// shot/tests leave no chrome in place.
    pub fn attach_chrome(&mut self, chrome: WindowChrome) {
        self.top_bar.apply_chrome(&chrome);
        self.chrome = Some(chrome);
        self.relayout();
    }

    /// Registers the OS shell integration (#320–#322). Only the real binary
    /// calls this; shot/tests leave the [`NullShell`] in place.
    pub fn attach_shell(&mut self, integration: Box<dyn ShellIntegration>) {
        self.shell_integration = integration;
    }

    /// Sets the one-line startup notice shown in the status area.
    pub fn set_backend_notice(&mut self, notice: impl Into<String>) {
        self.shell.set_backend_notice(notice);
    }

    /// Handles the shell's repaint timer.
    fn tick_inner(&mut self) {
        self.last_full_sync = Instant::now();
        let tick = self.shell.tick(Instant::now());
        self.update_shell();
        self.sync_views(tick.changes);
        self.schedule(tick.next_wake);
    }

    /// Publishes the current track and transport state to the OS shell, then
    /// drains any transport actions it queued.
    fn update_shell(&self) {
        let player = self.shell.player.as_ref();
        let info = player.now_playing();
        let track = info.and_then(|info| self.shell.library.track_by_path(&info.path));
        let meta = info.map(|info| NowPlaying {
            title: prefer(track, |track| &track.title, &info.title),
            artist: prefer(track, |track| &track.artist, &info.artist),
            album: prefer(track, |track| &track.album, &info.album),
            playing: player.status() == PlaybackStatus::Playing,
            position: player.position(),
            duration: player.duration(),
            path: Some(info.path.clone()),
        });
        self.shell_integration.now_playing(meta.as_ref());
        self.shell_integration
            .progress(meta.as_ref().and_then(NowPlaying::fraction));
        self.shell_integration.thumb_buttons(&[
            ThumbButton::Previous,
            ThumbButton::PlayPause,
            ThumbButton::Next,
        ]);
        self.shell_integration.poll();
    }

    /// Pushes the current shell state into the views and reflects the theme.
    fn sync_views(&mut self, changes: Changes) {
        let view = self.shell.state.view;
        let panels = self.shell.state.panels;
        if view != self.applied_view || panels != self.applied_panels {
            self.applied_view = view;
            self.applied_panels = panels;
            self.apply_visibility();
            self.relayout();
        }

        if view == View::Music {
            let playing_id = playing_id(self.shell.library.as_ref(), self.shell.player.as_ref());
            self.music.sync(
                &self.shell.state,
                self.shell.library.as_ref(),
                &self.shell.search,
                playing_id,
                changes,
            );
        } else if view == View::Folders {
            self.refresh_folders();
            let playing_id = playing_id(self.shell.library.as_ref(), self.shell.player.as_ref());
            self.folders.sync(
                &self.shell.state.folders,
                self.shell.library.as_ref(),
                playing_id,
            );
        }
        self.navigator.sync(view);

        // The transport and status models are shell state, synced from the
        // live player/library before the views read them.
        let player = self.shell.player.as_ref();
        self.shell.state.top_bar.sync(player);
        self.top_bar
            .sync(&self.shell.state.top_bar, &self.shell.state.search_query);
        self.shell.state.status_bar.sync(
            self.shell.state.search_result_count,
            self.shell.library.as_ref(),
            player,
        );
        let notice = self.shell.backend_notice();
        self.status_bar.sync(&self.shell.state.status_bar, notice);

        let look = (self.shell.state.theme, self.shell.state.accent);
        if look != self.applied_look {
            self.applied_look = look;
            self.ui.set_theme(app_theme(look.0, look.1));
        }
    }

    /// Shows the active central view and hides the others.
    fn apply_visibility(&self) {
        let view = self.shell.state.view;
        let music = view == View::Music;
        let folders = view == View::Folders;
        self.music.set_visible(music);
        self.folders.set_visible(folders);
        self.placeholder.set_visible(!music && !folders);
        self.navigator
            .set_visible(self.shell.state.panels.navigator);
        self.status_bar
            .set_visible(self.shell.state.panels.status_bar);
        if !music && !folders {
            self.placeholder.sync(&format!(
                "{} view: not ported to xui_core yet (see the migration epic #369)",
                view.label()
            ));
        }
    }

    /// Positions the caption band, the transport band, the status bar and the
    /// central area from the client rectangle.
    fn relayout(&mut self) {
        let client = self.ui.client_rect();
        let dpi = self.ui.dpi();
        let caption = self.caption_inset_px(dpi);
        let top_bar = dip(TOP_BAR_HEIGHT).to_px(dpi).value();
        let bar_top = client.top + caption;
        let bar_bottom = bar_top + top_bar;
        self.top_bar.set_bounds(
            Rect::new(client.left, client.top, client.right, bar_top),
            Rect::new(client.left, bar_top, client.right, bar_bottom),
        );

        let status = if self.shell.state.panels.status_bar {
            dip(STATUS_BAR_HEIGHT).to_px(dpi).value()
        } else {
            0
        };
        let bottom = client.bottom - status;
        self.status_bar
            .set_bounds(Rect::new(client.left, bottom, client.right, client.bottom));

        let top = bar_bottom;
        let navigator_width = if self.shell.state.panels.navigator {
            dip(self.shell.state.navigator_width).to_px(dpi).value()
        } else {
            0
        };
        self.navigator.set_bounds(Rect::new(
            client.left,
            top,
            client.left + navigator_width,
            bottom,
        ));
        let central = Rect::new(client.left + navigator_width, top, client.right, bottom);
        if self.shell.state.view == View::Music {
            self.music.set_bounds(central);
        } else if self.shell.state.view == View::Folders {
            self.folders.set_bounds(central);
        } else {
            self.placeholder.set_bounds(central);
        }
    }

    /// The caption band height in device pixels: the backend's reserved caption
    /// inset when the chrome is attached, else the requested constant.
    fn caption_inset_px(&self, dpi: u32) -> i32 {
        self.chrome.as_ref().map_or_else(
            || dip(CAPTION_HEIGHT).to_px(dpi).value(),
            |chrome| chrome.caption_inset().to_px(dpi).value(),
        )
    }

    /// Keeps a single repeating timer in step with the shell's `next_wake`.
    fn schedule(&mut self, next_wake: Option<Duration>) {
        let wanted = next_wake.map(|wait| wait.as_millis().min(u128::from(u32::MAX)) as u32);
        match wanted {
            Some(ms) if self.timer.is_none_or(|(_, current)| current != ms) => {
                if let Some((id, _)) = self.timer.take() {
                    self.ui.kill_timer(id);
                }
                let id = self.ui.set_timer(ms.max(1));
                if id.0 != 0 {
                    self.timer = Some((id, ms));
                }
            }
            None => {
                if let Some((id, _)) = self.timer.take() {
                    self.ui.kill_timer(id);
                }
            }
            Some(_) => {}
        }
    }

    /// Rebuilds the Folders model (its tree rows and visible ids) from the
    /// library snapshot.
    fn refresh_folders(&mut self) {
        let tracks: Vec<&TrackInfo> = self.shell.library.as_ref().tracks().iter().collect();
        let cx = Ctx::with_library(&tracks, None, self.shell.library.as_ref());
        self.shell.state.folders.refresh(&cx);
    }

    /// Applies a Folders intent through the shared model, dispatches the
    /// commands it emits, and refreshes the model.
    fn apply_folders(&mut self, message: FoldersMsg) {
        let tracks: Vec<&TrackInfo> = self.shell.library.as_ref().tracks().iter().collect();
        let cx = Ctx::with_library(&tracks, None, self.shell.library.as_ref());
        let mut out = Commands::new();
        self.shell.state.folders.update(message, &cx, &mut out);
        for command in out.into_vec() {
            self.shell.dispatch(command);
        }
        self.refresh_folders();
    }

    /// Runs one keyboard-shortcut action through the shared table helper.
    fn handle_shortcut(&mut self, action: ShortcutAction) {
        let player = self.shell.player.as_ref();
        let command = shortcut_command(
            action,
            player.position(),
            player.duration(),
            player.volume(),
        );
        if let Some(command) = command {
            self.shell.dispatch(command);
        }
    }
}

impl App for Win32App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Wake | Msg::Timer => self.tick_inner(),
            Msg::Resize => self.relayout(),
            Msg::Dispatch(command) => {
                self.shell.dispatch(command);
                self.tick_inner();
            }
            Msg::Navigate(view) => {
                self.shell.dispatch(Command::SetView(view));
                self.tick_inner();
            }
            Msg::PlayRow(row) => {
                let command = match self.shell.state.view {
                    View::Music => self.music.activate(row),
                    View::Folders => self.folders.activate(row),
                    _ => None,
                };
                if let Some(command) = command {
                    self.shell.dispatch(command);
                    self.tick_inner();
                }
            }
            Msg::ToggleStarRow(row) => {
                let command = match self.shell.state.view {
                    View::Music => self.music.toggle_star(row),
                    View::Folders => self.folders.toggle_star(row),
                    _ => None,
                };
                if let Some(command) = command {
                    self.shell.dispatch(command);
                    self.tick_inner();
                }
            }
            Msg::SortColumn(column) => {
                let Some(id) = track_table::column_id(column) else {
                    return;
                };
                match self.shell.state.view {
                    View::Music => {
                        self.shell.state.music.table.sort.toggle(id);
                        self.music.resort(
                            &self.shell.state,
                            self.shell.library.as_ref(),
                            &self.shell.search,
                        );
                    }
                    View::Folders => {
                        self.shell.state.folders.table.sort.toggle(id);
                        self.folders
                            .resort(&self.shell.state.folders, self.shell.library.as_ref());
                    }
                    _ => return,
                }
                self.tick_inner();
            }
            Msg::ContextRow(row) => {
                // The portable context menu is #376's; remember the row and log
                // where it would open.
                self.context_row = Some(row);
                tracing::debug!(row, "track context menu is not ported yet");
            }
            Msg::ContextAction(action) => {
                let Some(row) = self.context_row else {
                    return;
                };
                let track = match self.shell.state.view {
                    View::Music => self.music.track(row),
                    View::Folders => self.folders.track(row),
                    _ => None,
                };
                if let Some(track) = track
                    && let Some(command) = track_table::run_context_action(action, &track)
                {
                    self.shell.dispatch(command);
                    self.tick_inner();
                }
            }
            Msg::FoldersSelect(path) => {
                self.apply_folders(FoldersMsg::SelectNode(path));
                self.tick_inner();
            }
            Msg::FoldersSubfolders(include) => {
                self.apply_folders(FoldersMsg::SetIncludeSubfolders(include));
                self.tick_inner();
            }
            Msg::MusicShuffleAll => {
                let command = self.music.shuffle_all(
                    &self.shell.state,
                    self.shell.library.as_ref(),
                    &self.shell.search,
                );
                self.shell.dispatch(command);
                self.tick_inner();
            }
            Msg::Shortcut(action) => {
                self.handle_shortcut(action);
                self.tick_inner();
            }
            Msg::Shell(action) => {
                if let Some(command) = shell_command(action) {
                    self.shell.dispatch(command);
                    self.tick_inner();
                }
            }
            Msg::Minimize => {
                if let Some(chrome) = &self.chrome {
                    chrome.minimize();
                }
            }
            Msg::ToggleMaximize => {
                if let Some(chrome) = &self.chrome {
                    chrome.toggle_maximize();
                }
            }
            Msg::Quit => {
                self.shell.save_on_exit();
                ui.close();
                // The app owns the close decision (`on_close` returns
                // `Some(Msg::Quit)`), so nothing else ends the event loop.
                ui.quit();
            }
        }
    }
}

/// Maps an OS shell action to the shared transport command.
fn shell_command(action: ShellAction) -> Option<Command> {
    Some(match action {
        ShellAction::PlayPause => Command::PlayerPlayPause,
        ShellAction::Next => Command::PlayerNext,
        ShellAction::Previous => Command::PlayerPrevious,
        ShellAction::Stop => Command::PlayerStop,
        ShellAction::Seek(position) => Command::PlayerSeek(position),
    })
}

/// The library's tag for the current track, falling back to the player's
/// display string.
fn prefer(track: Option<&TrackInfo>, field: impl Fn(&TrackInfo) -> &str, fallback: &str) -> String {
    track
        .map(field)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

/// The library id of the player's current track, matched by path.
fn playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library
        .track_by_path(&now_playing.path)
        .map(|track| track.id)
}
