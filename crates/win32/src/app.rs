//! The native Win32 frontend's application object (#106).
//!
//! [`Win32App`] owns the toolkit-agnostic [`Shell`] and the window's controls.
//! `win32ui` drives it: every widget event is mapped to [`Msg`], delivered to
//! [`Win32App::update`], which runs [`Shell::tick`] on wakes and timers, syncs
//! the views, and schedules the next timer from [`Tick::next_wake`].

use std::time::Instant;

use emusic_ui::backend::ipc::IpcBridge;
use emusic_ui::config::Config;
use emusic_ui::library_api::{LibraryDataSource, StatsWindow};
use emusic_ui::panels::top_bar::TopBarMsg;
use emusic_ui::player_api::PlayerApi;
use emusic_ui::shell::{Changes, Shell};
use emusic_ui::state::{Command, View};
use emusic_ui::views::Commands;
use emusic_ui::views::Ctx;
use emusic_ui::views::column_browser::Pane;
use emusic_ui::views::folders::FoldersMsg;
use emusic_ui::views::now_playing::NowPlayingMsg;
use emusic_ui::waker::WakerSlot;
use win32ui::prelude::*;
use win32ui::{column, dip, row};

use crate::menu;
use crate::views::album_grid::{AlbumGridView, AlbumMsg};
use crate::views::artists::ArtistsView;
use crate::views::column_browser::ColumnBrowserView;
use crate::views::folders::FoldersView;
use crate::views::genres::GenresView;
use crate::views::history::HistoryView;
use crate::views::most_played::MostPlayedView;
use crate::views::music::MusicView;
use crate::views::navigator::NavigatorView;
use crate::views::now_playing::{self, NowPlayingView, SummaryEvent};
use crate::views::placeholder::Placeholder;
use crate::views::settings::{SettingsMsg, SettingsView};
use crate::views::starred::StarredView;
use crate::views::status_bar::StatusBarView;
use crate::views::top_bar::{self, TopBarView};
use crate::views::track_table::{self, ContextAction};
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
    /// Toggle the star of a track table row (a star-cell click).
    ToggleStarRow(usize),
    /// Sort the Music view by a header column.
    SortColumn(usize),
    /// Open the Music view's context menu for a row.
    ContextRow(usize),
    /// Run a track-table context-menu action.
    ContextAction(ContextAction),
    /// The Most Played view's time-window selector changed.
    MostPlayed(StatsWindow),
    /// Clear the whole play history, after the confirmation dialog.
    HistoryClear,
    /// An event from the Albums view.
    Album(AlbumMsg),
    /// A folder tree row was selected (or the selection cleared).
    FoldersSelect(String),
    /// The Folders view's "include subfolders" checkbox changed.
    FoldersSubfolders(bool),
    /// A folder tree row was right-clicked.
    FoldersContext(String),
    /// Start a scoped shuffle of a folder's tracks.
    FoldersShuffle(String),
    /// Run the "Shuffle play" action of the active name+counts view
    /// (Artists/Genres) on the row its context menu was opened on.
    NameCountShuffle,
    /// A column-browser pane's selection changed (the rows now selected).
    BrowserRow { pane: Pane, rows: Vec<usize> },
    /// An intent from the Settings view (folders, appearance, ...).
    Settings(SettingsMsg),
    /// Switch the central view (navigator row click).
    Navigate(View),
    /// A top-bar band event (transport button, toggle or slider).
    TopBar(TopBarEvent),
    /// The top-bar search box changed.
    TopBarSearch(String),
    /// A now-playing summary action (star, link, Properties, ...).
    NowPlaying(SummaryEvent),
    /// Jump to a queue preview row (double-click / Enter).
    QueueJump(usize),
    /// Open the queue's context menu for a preview row.
    QueueContext(usize),
    /// Remove the queue entry the context menu was opened on.
    QueueRemove,
    /// Close the window and exit.
    Quit,
}

/// The app: the shell plus the window's controls.
pub struct Win32App {
    shell: Shell,
    navigator: NavigatorView,
    central: Placeholder,
    browser: ColumnBrowserView,
    music: MusicView,
    albums: AlbumGridView,
    folders: FoldersView,
    artists: ArtistsView,
    genres: GenresView,
    most_played: MostPlayedView,
    history: HistoryView,
    settings: SettingsView,
    starred: StarredView,
    right_panel: NowPlayingView,
    status: StatusBarView,
    /// The top transport bar band, when the window is extended and DirectWrite
    /// is available.
    top_bar: Option<TopBarView>,
    /// The shell's repaint timer, if scheduled, and its interval in ms.
    timer: Option<(TimerId, u32)>,
    /// Panel visibility last applied, so a change triggers a relayout.
    applied_panels: emusic_ui::state::PanelVisibility,
    /// Theme last applied to the window, so a change re-themes it.
    applied_theme: emusic_ui::state::Theme,
    /// View last applied to the central area, so a change re-lays it out.
    applied_view: View,
    /// Whether the column browser was last shown, so a change re-lays it out.
    applied_browser_visible: bool,
    /// The column-browser toggle last reflected in the View menu's tick. Kept
    /// apart from [`Self::applied_browser_visible`], which is the *effective*
    /// visibility (also false off the Music view).
    applied_browser_toggle: bool,
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
        let browser = ColumnBrowserView::new(ui).expect("create column browser view");
        let music = MusicView::new(ui).expect("create music view");
        let albums = AlbumGridView::new(ui, waker.handle()).expect("create albums view");
        let folders = FoldersView::new(ui).expect("create folders view");
        let artists = ArtistsView::new(ui).expect("create artists view");
        let genres = GenresView::new(ui).expect("create genres view");
        let most_played = MostPlayedView::new(ui).expect("create most played view");
        let history = HistoryView::new(ui).expect("create history view");
        let settings = SettingsView::new(ui).expect("create settings view");
        let starred = StarredView::new(ui).expect("create starred view");
        let status = StatusBarView::new(ui).expect("create status bar");
        // The top bar needs an extended title bar (see `main`) and DirectWrite;
        // without them the app just runs without it.
        let top_bar = TopBarView::new(ui).ok();

        waker.bind(Win32Waker::new(ui.proxy()));
        // The panel's artwork cache decodes off-thread and wakes through `waker`.
        let right_panel = NowPlayingView::new(ui, waker.handle()).expect("create now playing view");
        let mut shell = Shell::new(library, player, config, config_path, waker);
        if let Some(ipc) = ipc {
            shell.attach_ipc(ipc);
        }
        if let Some(message) = startup {
            shell.handle_ipc_message(message);
        }

        ui.set_menu_bar(menu::build(&shell.state));
        // Only the active central view is placed by the layout (installed
        // below); the others are hidden so they keep no stale bounds.
        let view = shell.state.view;
        let browser_visible = view == View::Music && shell.state.music.browser.visible;
        central.set_visible(!matches!(
            view,
            View::Music
                | View::Albums
                | View::Artists
                | View::Folders
                | View::Genres
                | View::MostPlayed
                | View::Settings
                | View::Starred
                | View::History
        ));
        browser.set_visible(browser_visible);
        music.set_visible(view == View::Music);
        albums.set_visible(view == View::Albums);
        folders.set_visible(view == View::Folders);
        artists.set_visible(view == View::Artists);
        genres.set_visible(view == View::Genres);
        most_played.set_visible(view == View::MostPlayed);
        settings.set_visible(view == View::Settings);
        starred.set_visible(view == View::Starred);
        history.set_visible(view == View::History);
        ui.on_timer(|_| Some(Msg::Timer));

        let applied_panels = shell.state.panels;
        let applied_theme = shell.state.theme;
        let applied_view = shell.state.view;
        let applied_browser_toggle = shell.state.music.browser.visible;
        let mut app = Self {
            shell,
            navigator,
            central,
            browser,
            music,
            albums,
            folders,
            artists,
            genres,
            most_played,
            history,
            settings,
            starred,
            right_panel,
            status,
            top_bar,
            timer: None,
            applied_panels,
            applied_theme,
            applied_view,
            applied_browser_visible: browser_visible,
            applied_browser_toggle,
        };
        app.install_layout(ui, view);
        app.refresh_folders();
        app.refresh_music();
        app.tick(ui);
        app
    }

    /// Installs the window layout for `view`: the navigator, the active central
    /// area and the now-playing panel.
    ///
    /// Rebuilt on a view change so the Settings tab control only exists while
    /// Settings is shown (a tab node is always visible when installed).
    fn install_layout(&self, ui: &Ui<Msg>, view: View) {
        let central: LayoutItem = match view {
            View::Music => column![
                self.browser
                    .layout()
                    .height(dip(self.shell.state.music.browser.height)),
                self.music.fill(1),
            ]
            .fill(1),
            View::Albums => self.albums.layout().fill(1),
            View::Artists => self.artists.layout().fill(1),
            View::Genres => self.genres.layout().fill(1),
            View::MostPlayed => self.most_played.layout().fill(1),
            View::Folders => self.folders.layout().fill(1),
            View::Starred => self.starred.layout().fill(1),
            View::History => self.history.layout().fill(1),
            View::Settings => self.settings.tabs().into_layout_item(),
            _ => self.central.fill(1),
        };
        // An extended title bar reserves its strip, menu row and the top bar
        // band; content starts below `title_bar_height()`.
        let title_bar = ui.title_bar_height();
        let body = row![
            self.navigator.width(dip(220.0)),
            central,
            self.right_panel
                .layout()
                .width(dip(now_playing::PANEL_WIDTH)),
        ]
        .fill(1);
        // The material status bar is not a child: it reserves its band with a
        // bottom margin instead of taking a row.
        let bottom = self.status.bottom_margin(ui);
        let layout = match self.status.layout_item() {
            Some(child) => column![body, *child],
            None => column![body],
        };
        ui.set_layout(layout.margins(Insets::new(dip(0.0), title_bar, dip(0.0), bottom)));
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
        // Central-area routing: the Music list and the Albums grid own the
        // central area; every other view is still a placeholder.
        let view = self.shell.state.view;
        if view != self.applied_view {
            self.central.set_visible(!matches!(
                view,
                View::Music
                    | View::Albums
                    | View::Artists
                    | View::Folders
                    | View::Genres
                    | View::MostPlayed
                    | View::Settings
                    | View::Starred
            ));
            self.music.set_visible(view == View::Music);
            self.albums.set_visible(view == View::Albums);
            self.folders.set_visible(view == View::Folders);
            self.artists.set_visible(view == View::Artists);
            self.genres.set_visible(view == View::Genres);
            self.most_played.set_visible(view == View::MostPlayed);
            self.settings.set_visible(view == View::Settings);
            self.starred.set_visible(view == View::Starred);
            self.history.set_visible(view == View::History);
            let browser_visible = view == View::Music && self.shell.state.music.browser.visible;
            self.browser.set_visible(browser_visible);
            self.applied_browser_visible = browser_visible;
            self.applied_view = view;
            // Rebuild the layout so the active view's subtree (and, for
            // Settings, its tab control) is the one installed.
            self.install_layout(ui, view);
        }
        let mut relayout = false;

        // The Folders model rebuilds its tree rows and visible ids from the
        // library snapshot; refresh it before the view mirrors it.
        if changes.intersects(Changes::LIBRARY) {
            self.refresh_folders();
        }

        // The column-browser model rebuilds its cascading facets (and prunes
        // stale selections) from the library snapshot; the table and the panes
        // both read it.
        if changes.intersects(Changes::LIBRARY | Changes::SEARCH) {
            self.refresh_music();
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
        self.folders.sync(
            &self.shell.state.folders,
            self.shell.library.as_ref(),
            playing_id,
        );
        // The Starred model filters the whole library on every refresh, so it
        // is only synced while its view is showing; the first sync after
        // switching in rebuilds the rows (the model revision changed, or the
        // controls were never populated).
        if view == View::Starred {
            self.starred.sync(
                &mut self.shell.state,
                self.shell.library.as_ref(),
                playing_id,
            );
        }

        if view == View::Artists {
            self.artists
                .sync(&mut self.shell.state, self.shell.library.as_ref());
        }

        if view == View::Genres {
            self.genres
                .sync(&mut self.shell.state, self.shell.library.as_ref());
        }

        if view == View::MostPlayed {
            self.most_played.sync(
                &mut self.shell.state,
                self.shell.library.as_ref(),
                playing_id,
            );
        }

        // The History list is rebuilt when the library (and so the history)
        // changes; its day grouping and per-play status come from the shared
        // model and the current track.
        if view == View::History {
            let rebuild = changes.intersects(Changes::LIBRARY);
            self.history
                .sync(self.shell.library.as_ref(), playing_id, rebuild);
        }

        if view == View::Albums {
            let theme = ui.theme();
            if self.albums.sync(
                &mut self.shell.state,
                self.shell.library.as_ref(),
                playing_id,
                theme,
            ) {
                ui.relayout();
            }
        }

        // Show the browser only on the Music view and only when the model says
        // so (the menu's "Column browser" toggle); rebuild the layout if that
        // changed.
        let browser_visible = view == View::Music && self.shell.state.music.browser.visible;
        if browser_visible != self.applied_browser_visible {
            self.browser.set_visible(browser_visible);
            self.applied_browser_visible = browser_visible;
            relayout = true;
        }
        // The View menu ticks the column-browser *toggle*, which can change
        // even off the Music view; reinstall the bar when it does.
        let browser_toggle = self.shell.state.music.browser.visible;
        if browser_toggle != self.applied_browser_toggle {
            self.applied_browser_toggle = browser_toggle;
            ui.set_menu_bar(menu::build(&self.shell.state));
        }
        self.browser.sync(&self.shell.state.music.browser);

        if view == View::Settings
            && self
                .settings
                .sync(ui, &self.shell.state, self.shell.library.as_ref())
        {
            // The tab control has no runtime selection setter, so rebuild the
            // layout around the newly selected page.
            self.install_layout(ui, view);
        }

        if relayout {
            ui.relayout();
        }

        self.refresh_now_playing(ui.dpi());

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

        self.shell.state.top_bar.sync(self.shell.player.as_ref());
        if let Some(top_bar) = &mut self.top_bar {
            top_bar.sync(&self.shell.state.top_bar, &self.shell.state.search_query);
        }

        // Panel visibility is toggled through `Command::TogglePanel`; apply it
        // and relayout only when it actually changed.
        let panels = self.shell.state.panels;
        if panels != self.applied_panels {
            self.navigator.set_visible(panels.navigator);
            self.right_panel.set_visible(panels.right_panel);
            self.status.set_visible(panels.status_bar);
            ui.relayout();
            self.applied_panels = panels;
            // Rebuild the View menu so its ticks match the new visibility
            // (win32ui has no checked-setter).
            ui.set_menu_bar(menu::build(&self.shell.state));
        }

        // The dark/light menu toggle changes the shell's theme; mirror it onto
        // the window (the accent is a follow-up: win32ui's palette is richer).
        let theme = self.shell.state.theme;
        if theme != self.applied_theme {
            ui.set_theme(win32_theme(theme));
            self.applied_theme = theme;
        }
    }

    /// Applies a top-bar intent through the shared model and dispatches the
    /// commands it emits.
    fn apply_top_bar(&mut self, message: TopBarMsg) {
        let mut out = Commands::new();
        self.shell.state.top_bar.update(message, &mut out);
        for command in out.into_vec() {
            self.shell.dispatch(command);
        }
    }

    /// Refreshes the now-playing model from the player/library, then pushes it
    /// into the panel (artwork included).
    fn refresh_now_playing(&mut self, dpi: u32) {
        let shell = &mut self.shell;
        shell
            .state
            .now_playing
            .refresh(shell.player.as_ref(), shell.library.as_ref());
        self.right_panel.sync(&shell.state.now_playing, dpi);
    }

    /// Applies a now-playing intent through the shared model and dispatches any
    /// commands it emits.
    fn apply_now_playing(&mut self, message: NowPlayingMsg) {
        let mut out = Commands::new();
        self.shell.state.now_playing.update(message, &mut out);
        for command in out.into_vec() {
            self.shell.dispatch(command);
        }
    }

    /// Turns a summary click into a now-playing intent, if it carries one.
    fn handle_summary(&mut self, event: SummaryEvent) {
        if event == SummaryEvent::OpenFolder {
            let path = self
                .shell
                .state
                .now_playing
                .track()
                .map(|track| track.path.clone());
            if let Some(path) = path.filter(|path| !path.is_empty()) {
                open_file_location(&path);
            }
            return;
        }

        let message = {
            let model = &self.shell.state.now_playing;
            match event {
                SummaryEvent::ToggleStar => model
                    .track()
                    .map(|track| NowPlayingMsg::ToggleStar(track.id)),
                SummaryEvent::EditTags => {
                    model.track().map(|track| NowPlayingMsg::EditTags(track.id))
                }
                SummaryEvent::ShowProperties => model
                    .track()
                    .map(|track| NowPlayingMsg::ShowProperties(Box::new(track.clone()))),
                SummaryEvent::GoToArtist => model
                    .now_playing()
                    .filter(|np| !np.artist.is_empty())
                    .map(|np| NowPlayingMsg::GoToArtist(np.artist.clone())),
                SummaryEvent::GoToAlbum => {
                    model.track().filter(|t| !t.album.is_empty()).map(|track| {
                        NowPlayingMsg::GoToAlbum {
                            name: track.album.clone(),
                            artist: track.artist.clone(),
                        }
                    })
                }
                SummaryEvent::OpenFolder => None,
            }
        };
        if let Some(message) = message {
            self.apply_now_playing(message);
        }
    }

    /// Rebuilds the Folders view model (its tree rows and visible ids) from the
    /// library snapshot.
    fn refresh_folders(&mut self) {
        let tracks: Vec<&emusic_ui::library_api::TrackInfo> =
            self.shell.library.as_ref().tracks().iter().collect();
        let cx = Ctx::with_library(&tracks, None, self.shell.library.as_ref());
        self.shell.state.folders.refresh(&cx);
    }

    /// Applies a Folders intent through the shared model, dispatches the
    /// commands it emits, and refreshes the model.
    fn apply_folders(&mut self, message: FoldersMsg) {
        let tracks: Vec<&emusic_ui::library_api::TrackInfo> =
            self.shell.library.as_ref().tracks().iter().collect();
        let cx = Ctx::with_library(&tracks, None, self.shell.library.as_ref());
        let mut out = Commands::new();
        self.shell.state.folders.update(message, &cx, &mut out);
        for command in out.into_vec() {
            self.shell.dispatch(command);
        }
        self.refresh_folders();
    }

    /// Rebuilds the Music view model (its cascading column-browser facets and
    /// the filtered track list) from the library snapshot and the live search.
    fn refresh_music(&mut self) {
        let tracks: Vec<&emusic_ui::library_api::TrackInfo> =
            self.shell.library.as_ref().tracks().iter().collect();
        self.shell.state.music.refresh(&tracks, &self.shell.search);
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
                let command = match self.shell.state.view {
                    View::Music => self.music.activate(row),
                    View::Folders => self.folders.activate(row),
                    View::Starred => self.starred.activate(row),
                    View::MostPlayed => self.most_played.activate(row),
                    View::History => self.history.activate(row),
                    _ => None,
                };
                if let Some(command) = command {
                    self.shell.dispatch(command);
                    self.tick(ui);
                }
            }
            Msg::ToggleStarRow(row) => {
                let command = match self.shell.state.view {
                    View::Music => self.music.toggle_star(row),
                    View::Folders => self.folders.toggle_star(row),
                    View::Starred => self.starred.toggle_star(row),
                    View::MostPlayed => self.most_played.toggle_star(row),
                    _ => None,
                };
                if let Some(command) = command {
                    self.shell.dispatch(command);
                    self.tick(ui);
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
                    View::Starred => {
                        self.shell.state.starred.table.sort.toggle(id);
                        self.starred
                            .resort(&self.shell.state, self.shell.library.as_ref());
                    }
                    View::MostPlayed => {
                        self.shell.state.most_played.table.sort.toggle(id);
                        self.most_played
                            .resort(&self.shell.state, self.shell.library.as_ref());
                    }
                    _ => return,
                }
                self.tick(ui);
            }
            Msg::ContextRow(row) => {
                let menu = match self.shell.state.view {
                    View::Music => {
                        self.music.set_context_row(row);
                        self.music.context_menu().clone()
                    }
                    View::Folders => {
                        self.folders.set_context_row(row);
                        self.folders.context_menu().clone()
                    }
                    View::Artists => {
                        self.artists.set_context_row(row);
                        self.artists.context_menu().clone()
                    }
                    View::Genres => {
                        self.genres.set_context_row(row);
                        self.genres.context_menu().clone()
                    }
                    View::Starred => {
                        self.starred.set_context_row(row);
                        self.starred.context_menu().clone()
                    }
                    View::MostPlayed => {
                        self.most_played.set_context_row(row);
                        self.most_played.context_menu().clone()
                    }
                    View::History => {
                        if !self.history.is_entry_row(row) {
                            return;
                        }
                        self.history.set_context_row(row);
                        self.history.context_menu().clone()
                    }
                    _ => return,
                };
                ui.popup(&menu, ui.cursor_position());
            }
            Msg::NameCountShuffle => {
                let view = self.shell.state.view;
                let name = match view {
                    View::Artists => self.artists.context_name(),
                    View::Genres => self.genres.context_name(),
                    _ => None,
                };
                let Some(name) = name else {
                    return;
                };
                let commands = match view {
                    View::Artists => self.artists.shuffle(
                        name,
                        &mut self.shell.state,
                        self.shell.library.as_ref(),
                    ),
                    View::Genres => self.genres.shuffle(
                        name,
                        &mut self.shell.state,
                        self.shell.library.as_ref(),
                    ),
                    _ => return,
                };
                for command in commands {
                    self.shell.dispatch(command);
                }
                self.tick(ui);
            }
            Msg::ContextAction(action) => {
                let command = match self.shell.state.view {
                    View::Music => self.music.run_context(action, ui.hwnd()),
                    View::Folders => self.folders.run_context(action, ui.hwnd()),
                    View::Starred => self.starred.run_context(action, ui.hwnd()),
                    View::MostPlayed => self.most_played.run_context(action, ui.hwnd()),
                    View::History => self.history.run_context(action),
                    _ => None,
                };
                if let Some(command) = command {
                    self.shell.dispatch(command);
                    self.tick(ui);
                }
            }
            Msg::MostPlayed(window) => {
                self.shell.state.most_played.window = window;
                self.tick(ui);
            }
            Msg::Album(msg) => {
                let playing_id =
                    playing_id(self.shell.library.as_ref(), self.shell.player.as_ref());
                let mut commands = Commands::new();
                self.albums.update(
                    msg,
                    &mut self.shell.state,
                    self.shell.library.as_ref(),
                    playing_id,
                    ui,
                    &mut commands,
                );
                for command in commands.into_vec() {
                    self.shell.dispatch(command);
                }
                self.tick(ui);
            }
            Msg::FoldersSelect(path) => {
                self.apply_folders(FoldersMsg::SelectNode(path));
                self.tick(ui);
            }
            Msg::FoldersSubfolders(on) => {
                self.apply_folders(FoldersMsg::SetIncludeSubfolders(on));
                self.tick(ui);
            }
            Msg::FoldersContext(path) => {
                let menu = self.folders.shuffle_menu(&path);
                ui.popup(&menu, ui.cursor_position());
            }
            Msg::FoldersShuffle(path) => {
                let recursive = self.shell.state.folders.include_subfolders;
                self.apply_folders(FoldersMsg::Shuffle { path, recursive });
                self.tick(ui);
            }
            Msg::BrowserRow { pane, rows } => {
                if crate::views::column_browser::apply_selection(
                    &mut self.shell.state.music.browser,
                    pane,
                    &rows,
                ) {
                    self.refresh_music();
                    self.tick(ui);
                }
            }
            Msg::Settings(msg) => {
                let mut commands = Commands::new();
                self.settings
                    .update(msg, ui, &mut self.shell.state, &mut commands);
                for command in commands.into_vec() {
                    self.shell.dispatch(command);
                }
                self.tick(ui);
            }
            Msg::Navigate(view) => {
                self.shell.dispatch(Command::SetView(view));
                self.tick(ui);
            }
            Msg::TopBar(event) => {
                if let Some(message) = top_bar::to_message(event) {
                    self.apply_top_bar(message);
                }
                self.tick(ui);
            }
            Msg::TopBarSearch(query) => {
                self.apply_top_bar(TopBarMsg::SetSearchQuery(query));
                self.tick(ui);
            }
            Msg::NowPlaying(event) => {
                self.handle_summary(event);
                self.tick(ui);
            }
            Msg::QueueJump(row) => {
                if let Some(index) = self.right_panel.queue_index(row) {
                    self.apply_now_playing(NowPlayingMsg::QueueJump(index));
                    self.tick(ui);
                }
            }
            Msg::QueueContext(row) => {
                self.right_panel.set_context_row(row);
                ui.popup(self.right_panel.context_menu(), ui.cursor_position());
            }
            Msg::QueueRemove => {
                if let Some(index) = self.right_panel.context_index() {
                    self.apply_now_playing(NowPlayingMsg::QueueRemove(index));
                    self.tick(ui);
                }
            }
            Msg::HistoryClear => {
                if self.history.confirm_clear(ui) {
                    self.shell.dispatch(Command::HistoryClear);
                    self.tick(ui);
                }
            }
            Msg::Quit => ui.close(),
        }
    }
}

/// Reveals `path` in Explorer.
fn open_file_location(path: &str) {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.replace('/', "\\")))
        .spawn();
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
