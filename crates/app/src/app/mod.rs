//! The application object (#106), on the portable `xui_core` runtime.
//!
//! [`Win32App`] owns the toolkit-agnostic [`Shell`] and the window's views.
//! `xui_core` drives it: widget events map to [`Msg`], the runtime calls
//! [`Win32App::update`], which runs [`Shell::tick`] on wakes and timers, syncs
//! the views, and schedules the next timer from `Tick::next_wake`.
//!
//! Every central view is ported to the portable widget layer (#370–#376). OS
//! shell services go through [`emusic_platform`], whose signature names no
//! backend type.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use emusic_platform::{NowPlaying, NullShell, ShellIntegration, ThumbButton};
use emusic_ui::backend::ipc::IpcBridge;
use emusic_ui::config::Config;
use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::player_api::{PlaybackStatus, PlayerApi};
use emusic_ui::shell::Shell;
use emusic_ui::state::View;
use emusic_ui::waker::WakerSlot;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::{Event, TimerId, WidgetId};
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::{Dialog, Menu};

use crate::menu;
use crate::theme::app_theme;
use crate::views::album_grid::AlbumGridView;
use crate::views::artists::ArtistsView;
use crate::views::column_browser::ColumnBrowserView;
use crate::views::folders::FoldersView;
use crate::views::genres::GenresView;
use crate::views::history::HistoryView;
use crate::views::most_played::MostPlayedView;
use crate::views::music::MusicView;
use crate::views::navigator::NavigatorView;
use crate::views::now_playing::NowPlayingView;
use crate::views::settings::SettingsView;
use crate::views::starred::StarredView;
use crate::views::status_bar::StatusBarView;
use crate::views::top_bar::TopBarView;
use crate::views::visualization::VisualizationView;
use crate::waker::UiWaker;
use crate::window::WindowChrome;

mod dialogs;
mod layout;
mod msg;
mod sync;
mod update;

pub use msg::Msg;

use dialogs::TagEditorSession;

/// The app: the shell plus the window's views.
pub struct Win32App {
    shell: Shell,
    ui: Ui<Msg>,
    navigator: NavigatorView,
    top_bar: TopBarView,
    status_bar: StatusBarView,
    music: MusicView,
    folders: FoldersView,
    albums: AlbumGridView,
    artists: ArtistsView,
    genres: GenresView,
    starred: StarredView,
    most_played: MostPlayedView,
    history: HistoryView,
    /// The Music view's three cascading facet lists (#372).
    browser: ColumnBrowserView,
    settings: SettingsView,
    /// The Now Playing central view (#371).
    now_playing: NowPlayingView,
    /// The Visualization central view: the portable preset browser (#371).
    visualization: VisualizationView,
    /// The File/View/Help menu bar, with its ticks kept in step with the state.
    menu: Menu<Msg>,
    /// The track table's pooled right-click menu.
    context_menu: Menu<Msg>,
    /// The Now Playing queue's right-click menu.
    queue_context: Menu<Msg>,
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
    /// Column-browser visibility last applied, so toggling it re-lays out.
    applied_browser_visible: bool,
    /// Theme and accent last applied to the window, so a change re-themes it.
    applied_look: (emusic_ui::state::Theme, emusic_ui::state::Accent),
    /// When the views were last fully synced.
    last_full_sync: Instant,
    /// The central area last laid out, so a context menu opens near it.
    central_bounds: Rect,
    /// The row the track context menu was opened on, while a context action is
    /// pending.
    context_row: Option<usize>,
    /// The queue preview row the queue context menu was opened on.
    queue_context_row: Option<usize>,
    /// The Help -> Keyboard shortcuts message dialog, while it is open.
    shortcuts_dialog: Option<Dialog<Msg>>,
    /// The History -> Clear confirmation, while it is open.
    history_dialog: Option<Dialog<Msg>>,
    /// The open tag editor's session, while its window is up (#376).
    tag_editor: Option<TagEditorSession>,
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
        mock: bool,
    ) -> Self {
        let look = (config.theme, config.accent);
        ui.set_theme(app_theme(look.0, look.1));
        // The Library settings page adds folders through the off-thread picker;
        // installing its result channel once is a no-op after the first call.
        emusic_ui::folder_picker::init();

        let navigator = NavigatorView::new(ui);
        let top_bar = TopBarView::new(ui);
        let status_bar = StatusBarView::new(ui);
        let music = MusicView::new(ui);
        let folders = FoldersView::new(ui);
        let albums = AlbumGridView::new(ui, waker.handle()).expect("create albums view");
        let artists = ArtistsView::new(ui);
        let genres = GenresView::new(ui);
        let starred = StarredView::new(ui);
        let most_played = MostPlayedView::new(ui);
        let history = HistoryView::new(ui);
        let browser = ColumnBrowserView::new(ui);
        let settings = SettingsView::new(ui);
        let now_playing = NowPlayingView::new(ui, waker.handle());
        let visualization = VisualizationView::new(ui, mock, waker.handle());

        waker.bind(UiWaker::new(ui.proxy()));
        let mut shell = Shell::new(library, player, config, config_path, waker);
        if let Some(ipc) = ipc {
            shell.attach_ipc(ipc);
        }
        if !startup_files.is_empty() {
            shell.player.replace_and_play(&startup_files, 0);
        }

        let menu = menu::bar(ui, Rect::default(), &shell.state);
        let context_menu = menu::track_context(ui);
        let queue_context = menu::queue_context(ui);

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
        let applied_browser_visible =
            applied_view == View::Music && shell.state.music.browser.visible;
        let mut app = Win32App {
            shell,
            ui: ui.clone(),
            navigator,
            top_bar,
            status_bar,
            music,
            folders,
            albums,
            artists,
            genres,
            starred,
            most_played,
            history,
            browser,
            settings,
            now_playing,
            visualization,
            menu,
            context_menu,
            queue_context,
            chrome: None,
            shell_integration: Box::new(NullShell),
            timer: None,
            applied_view,
            applied_panels,
            applied_browser_visible,
            applied_look: look,
            last_full_sync: Instant::now(),
            central_bounds: Rect::default(),
            context_row: None,
            queue_context_row: None,
            shortcuts_dialog: None,
            history_dialog: None,
            tag_editor: None,
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
