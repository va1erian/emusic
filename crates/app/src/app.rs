//! The application object (#106).
//!
//! [`Win32App`] owns the toolkit-agnostic [`Shell`] and the window's controls.
//! `win32ui` drives it: every widget event is mapped to [`Msg`], delivered to
//! [`Win32App::update`], which runs [`Shell::tick`] on wakes and timers, syncs
//! the views, and schedules the next timer from [`Tick::next_wake`].

use std::path::PathBuf;
use std::time::{Duration, Instant};

use emusic_ui::backend::ipc::IpcBridge;
use emusic_ui::config::Config;
use emusic_ui::library_api::{LibraryDataSource, StatsWindow};
use emusic_ui::panels::top_bar::TopBarMsg;
use emusic_ui::panels::visualizer::FRAME_INTERVAL;
use emusic_ui::player_api::{PlaybackStatus, PlayerApi};
use emusic_ui::shell::{Changes, Shell};
use emusic_ui::state::projectm::{ProjectMAvailability, VizDock, VizSurface};
use emusic_ui::state::{
    AppState, Appearance, Command, MAX_NAVIGATOR_WIDTH, MAX_RIGHT_PANEL_WIDTH, MIN_NAVIGATOR_WIDTH,
    MIN_RIGHT_PANEL_WIDTH, View, VisualizerMode, VizCommand, WindowGeometry,
};
use emusic_ui::views::Commands;
use emusic_ui::views::Ctx;
use emusic_ui::views::column_browser::{
    MAX_HEIGHT as BROWSER_MAX_HEIGHT, MIN_HEIGHT as BROWSER_MIN_HEIGHT, Pane,
};
use emusic_ui::views::folders::FoldersMsg;
use emusic_ui::views::now_playing::NowPlayingMsg;
use emusic_ui::waker::WakerSlot;
use win32ui::prelude::*;
use win32ui::{column, dip, split_col, split_row};

use crate::backend::smtc::Smtc;
use crate::backend::taskbar::TaskbarPreview;
use crate::backend::thumbbar::ThumbBar;
use crate::dialogs::database_info::{self, DatabaseInfoChoice};
use crate::dialogs::properties;
use crate::dialogs::tag_editor;
use crate::menu;
use crate::theme::win32_theme;
use crate::views::album_grid::{AlbumGridView, AlbumMsg};
use crate::views::artists::ArtistsView;
use crate::views::column_browser::ColumnBrowserView;
use crate::views::folders::FoldersView;
use crate::views::genres::GenresView;
use crate::views::history::HistoryView;
use crate::views::most_played::MostPlayedView;
use crate::views::music::MusicView;
use crate::views::navigator::NavigatorView;
use crate::views::now_playing::{CentralNowPlayingView, NowPlayingView, SummaryEvent};
use crate::views::placeholder::Placeholder;
use crate::views::preset_browser::{self, PresetBrowserView};
use crate::views::projectm::{
    GraceTimer, PresetFiles, PresetRoots, PresetScanner, ProjectMEvent, VizWindow,
};
use crate::views::settings::{SettingsMsg, SettingsView};
use crate::views::starred::StarredView;
use crate::views::status_bar::StatusBarView;
use crate::views::top_bar::{self, TopBarView};
use crate::views::track_table::{self, ContextAction};
use crate::waker::Win32Waker;

/// How often the views are fully synced while the visualizer animates faster.
const FULL_SYNC_INTERVAL: Duration = Duration::from_millis(250);

/// Minimum height of the Music view's table under the column-browser splitter,
/// in DIP (#342).
const TABLE_MIN_HEIGHT: f32 = 160.0;
/// Minimum width of the central area between the navigator and right panel, in
/// DIP (#342).
const MIDDLE_MIN_WIDTH: f32 = 320.0;

/// Everything the window can ask the app to do.
pub enum Msg {
    /// A background worker (search, IPC, image decode) woke the UI.
    Wake,
    /// The shell's repaint timer fired.
    Timer,
    /// Apply a command from the menu bar.
    Dispatch(Command),
    /// Open File -> Database info.
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
    /// The Music view's column-browser splitter moved (#342).
    BrowserSplit(Dip),
    /// The navigator's splitter moved (#342).
    NavigatorSplit(Dip),
    /// The now-playing panel's splitter moved (#342).
    RightPanelSplit(Dip),
    /// An intent from the Settings view (folders, appearance, ...).
    Settings(SettingsMsg),
    /// Switch the central view (navigator row click).
    Navigate(View),
    /// The navigator's row for `View` was right-clicked.
    NavigatorContext(View),
    /// Start a shuffled playback over the whole library (navigator context
    /// menu on the Music row, #242).
    NavigatorShuffleAll,
    /// Start a shuffled playback over the Music view's currently visible
    /// tracks (its header's "Shuffle all" button, #242).
    MusicShuffleAll,
    /// A top-bar band event (transport button, toggle or slider).
    TopBar(TopBarEvent),
    /// The top-bar search box changed.
    TopBarSearch(String),
    /// A now-playing summary action (star, link, Properties, ...).
    NowPlaying(SummaryEvent),
    /// The tag editor dialog left a save request in its bridge (#278).
    TagEditorApply,
    /// Jump to a right-panel queue preview row (double-click / Enter).
    QueueJump(usize),
    /// Open the right panel's queue context menu for a preview row.
    QueueContext(usize),
    /// Remove the queue entry the right panel's context menu was opened on.
    QueueRemove,
    /// Jump to a central Now Playing view queue preview row (#247;
    /// double-click / Enter). Distinct from [`Msg::QueueJump`] so the two
    /// queue lists, both visible at once, route independently.
    CentralQueueJump(usize),
    /// Open the central Now Playing view queue's context menu for a preview
    /// row (#247).
    CentralQueueContext(usize),
    /// Remove the queue entry the central view's context menu was opened on
    /// (#247).
    CentralQueueRemove,
    /// The top-bar visualizer strip was clicked: cycle its mode.
    CycleVisualizer,
    /// A projectM surface gesture (hover button, double-click) as a command.
    Viz(VizCommand),
    /// The visualization window reported its engine status (#303).
    VizAvailability(ProjectMAvailability),
    /// Open the projectM surface's right-click menu (#306).
    VizMenu,
    /// The preset browser's filter box changed (#338).
    PresetFilter(String),
    /// A preset-browser row was selected (single click).
    PresetSelect(usize),
    /// A preset-browser row was activated (double-click / Enter): play it.
    PresetPlay(usize),
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
    preset_browser: PresetBrowserView,
    settings: SettingsView,
    starred: StarredView,
    right_panel: NowPlayingView,
    /// The `View::NowPlaying` central view (#247): the same summary and
    /// queue widgets as `right_panel`, laid out full width. The right panel
    /// keeps showing while this is active — see `install_layout`.
    now_playing_central: CentralNowPlayingView,
    status: StatusBarView,
    /// The top transport bar band, when the window is extended and DirectWrite
    /// is available.
    top_bar: Option<TopBarView>,
    /// The shell's repaint timer, if scheduled, and its interval in ms.
    timer: Option<(TimerId, u32)>,
    /// Panel visibility last applied, so a change triggers a relayout.
    applied_panels: emusic_ui::state::PanelVisibility,
    /// Whether the projectM panel row was last expanded, so a change triggers
    /// a relayout (#302).
    applied_viz_visible: bool,
    /// Whether the projectM surface is actually rendering this tick: shown,
    /// not collapsed and the window not minimised (#305).
    viz_active: bool,
    /// Whether the *panel* surface is the one running (so the frame-rate wake
    /// can fall back while the independent window keeps running, #303).
    viz_panel_active: bool,
    /// Whether the independent visualization window should be shown and fed
    /// (#303).
    viz_window_active: bool,
    /// The independent visualization window, while it is open (#303). It is
    /// kept open (hidden) while the surface is elsewhere, so its instance and
    /// placement survive a toggle.
    viz_window: Option<VizWindow>,
    /// Set once opening the visualization window failed, so it is not retried
    /// every frame.
    viz_window_failed: bool,
    /// Keeps a stopped projectM instance alive briefly so a quick toggle
    /// resumes without rebuilding it (#305).
    viz_grace: GraceTimer,
    /// The background preset scan, while one is running (#305).
    preset_scanner: Option<PresetScanner>,
    /// The scanned preset files, cached so a surface opened after the scan
    /// (the independent window, #303) still receives them.
    preset_files: Option<PresetFiles>,
    /// Bumped whenever `preset_files` changes, so the preset browser (#338)
    /// rebuilds only on a finished scan and not on every tick.
    preset_scan_generation: u64,
    /// The preset configuration (disabled packs, user folder) the current scan
    /// was started for, so a settings change restarts it (#305).
    preset_config: Option<(Vec<String>, Option<PathBuf>)>,
    /// Theme and accent last applied to the window, so a change re-themes it.
    applied_look: (emusic_ui::state::Theme, emusic_ui::state::Accent),
    /// Font size, density and zebra last applied, so a change relayouts once
    /// (#309).
    applied_appearance: Appearance,
    /// The DPI last applied to the views, so a monitor move rebuilds fonts
    /// (#309).
    applied_dpi: u32,
    /// View last applied to the central area, so a change re-lays it out.
    applied_view: View,
    /// Whether the column browser was last shown, so a change re-lays it out.
    applied_browser_visible: bool,
    /// The column-browser toggle last reflected in the View menu's tick. Kept
    /// apart from [`Self::applied_browser_visible`], which is the *effective*
    /// visibility (also false off the Music view).
    applied_browser_toggle: bool,
    /// The Visualization menu ticks last applied: shown, locked, panel,
    /// window, fullscreen (#306). Updated in place so the bar is not rebuilt.
    applied_viz_menu: (bool, bool, bool, bool, bool),
    /// When the views were last fully synced, so visualizer frames in between
    /// can skip the (much costlier) full sync.
    last_full_sync: Instant,
    /// The open tag editor's bridge, while its modal dialog is up (#278).
    tag_editor: Option<std::rc::Rc<std::cell::RefCell<tag_editor::Bridge>>>,
    /// OS media controls / hardware media keys (#320). `None` in
    /// `emusic-shot` and tests, so headless runs never touch SMTC.
    smtc: Option<Smtc>,
    /// Windows taskbar thumbnail-toolbar transport buttons (#321). `None` in
    /// `emusic-shot` and tests, so headless runs never touch the shell.
    thumbbar: Option<ThumbBar>,
    /// DWM iconic taskbar preview + taskbar progress bar (#322). `None` in
    /// `emusic-shot` and tests, so headless runs never touch DWM or the shell.
    taskbar_preview: Option<TaskbarPreview>,
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
        // Install the appearance metrics before any view is created, so the
        // controls and the custom widgets are built with the saved font size,
        // density and zebra flag (#309).
        let appearance = config.appearance;
        let dpi = ui.dpi();
        crate::appearance::install(appearance, dpi);
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
        let preset_browser = PresetBrowserView::new(ui).expect("create preset browser view");
        let settings =
            SettingsView::new(ui, config.visualizer_enabled).expect("create settings view");
        let starred = StarredView::new(ui).expect("create starred view");
        let status = StatusBarView::new(ui).expect("create status bar");
        // The top bar needs an extended title bar (see `main`) and DirectWrite;
        // without them the app just runs without it.
        let top_bar = TopBarView::new(ui).ok();

        waker.bind(Win32Waker::new(ui.proxy()));
        // Each surface's artwork cache decodes off-thread and wakes through
        // its own handle to the same `waker`.
        let right_panel = NowPlayingView::new(ui, waker.handle()).expect("create now playing view");
        let now_playing_central = CentralNowPlayingView::new(ui, waker.handle())
            .expect("create now playing central view");
        let mut shell = Shell::new(library, player, config, config_path, waker);
        if let Some(ipc) = ipc {
            shell.attach_ipc(ipc);
        }
        if let Some(message) = startup {
            shell.handle_ipc_message(message);
        }

        // Restore the window geometry saved on the previous exit (#214); the
        // placement is applied before the window is first shown, overriding the
        // default centring `win32ui` chose. Route the close button through
        // `Msg::Quit` so the config (with the final geometry) is saved on exit.
        apply_saved_geometry(ui, shell.state.window);
        ui.on_close(|| Some(Msg::Quit));
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
                | View::NowPlaying
                | View::Visualization
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
        preset_browser.set_visible(view == View::Visualization);
        now_playing_central.set_visible(view == View::NowPlaying);
        // Panel visibility is otherwise only applied by `tick` when it
        // *changes*, so a panel disabled in the saved config must be hidden
        // here or the first tick would skip it and the panel would stay on.
        navigator.set_visible(shell.state.panels.navigator);
        right_panel.set_visible(shell.state.panels.right_panel);
        right_panel
            .set_queue_visible(shell.state.panels.right_panel && shell.state.panels.next_tracks);
        status.set_visible(shell.state.panels.status_bar);
        ui.on_timer(|_| Some(Msg::Timer));

        let applied_panels = shell.state.panels;
        let applied_look = (shell.state.theme, shell.state.accent);
        let applied_appearance = shell.state.appearance;
        // `0` can never equal a real DPI, so the first sync applies the
        // appearance to the lists (whose controls were created with the
        // system font before this).
        let applied_dpi = 0;
        let applied_view = shell.state.view;
        let applied_browser_toggle = shell.state.music.browser.visible;
        let applied_viz_menu = viz_menu_ticks(&shell.state.projectm);
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
            preset_browser,
            settings,
            starred,
            right_panel,
            now_playing_central,
            status,
            top_bar,
            timer: None,
            applied_panels,
            applied_viz_visible: false,
            viz_active: false,
            viz_panel_active: false,
            viz_window_active: false,
            viz_window: None,
            viz_window_failed: false,
            viz_grace: GraceTimer::default(),
            preset_scanner: None,
            preset_files: None,
            preset_scan_generation: 0,
            preset_config: None,
            applied_look,
            applied_appearance,
            applied_dpi,
            applied_view,
            applied_browser_visible: browser_visible,
            applied_browser_toggle,
            applied_viz_menu,
            last_full_sync: Instant::now(),
            tag_editor: None,
            smtc: None,
            thumbbar: None,
            taskbar_preview: None,
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
        let state = &self.shell.state;
        let central: LayoutItem = match view {
            View::Music => {
                // The column-browser strip sits above the table, separated by a
                // draggable splitter (#342) seeded from the saved height.
                let rest = column![self.music.header(), self.music.fill(1)];
                split_col![self.browser.layout(), rest]
                    .position(dip(state.music.browser.height))
                    .min(dip(BROWSER_MIN_HEIGHT), dip(TABLE_MIN_HEIGHT))
                    .on_moved(|position| Some(Msg::BrowserSplit(position)))
                    .into_layout_item()
            }
            View::Albums => self.albums.layout().fill(1),
            View::Artists => self.artists.layout().fill(1),
            View::Genres => self.genres.layout().fill(1),
            View::MostPlayed => self.most_played.layout().fill(1),
            View::Folders => self.folders.layout().fill(1),
            View::Starred => self.starred.layout().fill(1),
            View::History => self.history.layout().fill(1),
            View::Visualization => self.preset_browser.layout().fill(1),
            View::NowPlaying => self.now_playing_central.layout().fill(1),
            View::Settings => self.settings.tabs().into_layout_item(),
        };
        // An extended title bar reserves its strip, menu row and the top bar
        // band; content starts below `title_bar_height()`.
        let title_bar = ui.title_bar_height();
        // The navigator and the now-playing panel are draggable splitters
        // (#342). The panel is anchored from the end (`position_b`), so it
        // keeps its saved width while the central area grows with the window.
        let middle = split_row![central, self.right_panel.layout()]
            .position_b(dip(state.right_panel_width))
            .min(dip(MIDDLE_MIN_WIDTH), dip(MIN_RIGHT_PANEL_WIDTH))
            .on_moved(|position| Some(Msg::RightPanelSplit(position)));
        let body = split_row![self.navigator, middle]
            .position(dip(state.navigator_width))
            .min(dip(MIN_NAVIGATOR_WIDTH), dip(MIDDLE_MIN_WIDTH))
            .on_moved(|position| Some(Msg::NavigatorSplit(position)))
            .into_layout_item();
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

    /// Registers the OS media-control integration (#320). Only the real binary
    /// calls this; shot/tests leave it unset so they never touch SMTC.
    pub fn attach_smtc(&mut self, smtc: Smtc) {
        self.smtc = Some(smtc);
    }

    /// Registers the Windows taskbar thumbnail-toolbar buttons (#321). Only the
    /// real binary calls this; shot/tests leave it unset so they never touch
    /// the shell.
    pub fn attach_thumbbar(&mut self, thumbbar: ThumbBar) {
        self.thumbbar = Some(thumbbar);
    }

    /// Registers the DWM iconic taskbar preview and progress bar (#322). Only
    /// the real binary calls this; shot/tests leave it unset so they never
    /// touch DWM or the shell.
    pub fn attach_taskbar_preview(&mut self, preview: TaskbarPreview) {
        self.taskbar_preview = Some(preview);
    }

    /// Handles the shell's repaint timer. While the visualizer animates the
    /// timer fires at [`FRAME_INTERVAL`](emusic_ui::panels::visualizer::FRAME_INTERVAL);
    /// most of those frames only need the strip fed, so the full sync (every
    /// view, the model refreshes) runs at a lower rate.
    fn on_timer(&mut self, ui: &mut Ui<Msg>) {
        let animating = self.shell.state.visualizer_enabled
            && self.shell.state.visualizer != VisualizerMode::Off;
        if animating && self.last_full_sync.elapsed() < FULL_SYNC_INTERVAL {
            if let Some(top_bar) = &self.top_bar {
                top_bar.feed_visualizer(self.shell.state.visualizer, self.shell.player.as_ref());
            }
            return;
        }
        self.tick(ui);
    }

    /// Runs the shell for this frame, syncs the views and schedules the timer.
    fn tick(&mut self, ui: &mut Ui<Msg>) {
        self.last_full_sync = Instant::now();
        // Remember the live window geometry for the next launch (#214), before
        // the shell's persistence pass captures the state. The independent
        // visualization window records its own (#303).
        record_window_geometry(ui, &mut self.shell.state);
        self.record_viz_geometry();
        // Mirror the current track to the OS media overlay and fold the
        // overlay's transport events into this tick's queued commands (#320),
        // then mirror the player's transport state onto the taskbar thumbnail
        // buttons and fold their presses in the same way (#321). Both push
        // into `shell.state.pending`, which `shell.tick` applies below.
        if let Some(smtc) = self.smtc.as_mut() {
            smtc.sync(
                self.shell.player.as_ref(),
                self.shell.library.as_ref(),
                &mut self.shell.state,
            );
        }
        if let Some(thumbbar) = self.thumbbar.as_mut() {
            thumbbar.sync(self.shell.player.as_ref(), &mut self.shell.state);
        }
        let tick = self.shell.tick(Instant::now());
        // Only now are this frame's queued commands applied, so decide here
        // whether a visualization surface runs. The shell computed its wake
        // from the previous `running`, so a surface that just became active is
        // given the frame-rate wake back below (#303, #305).
        let viz_wake = self.update_viz_active(ui, tick.next_wake);
        // Keep the modal tag editor posted on its save (the shell delivers
        // outcomes into `state.tag_editor`; the dialog polls the bridge).
        if let (Some(bridge), Some(editor)) = (&self.tag_editor, &self.shell.state.tag_editor) {
            bridge.borrow_mut().status = editor.status.clone();
        }
        self.sync_views(ui, tick.changes);
        // The taskbar preview reads the now-playing model that `sync_views`
        // just refreshed, so it runs after it (#322).
        if let Some(preview) = self.taskbar_preview.as_mut() {
            preview.sync(self.shell.player.as_ref(), &self.shell.state.now_playing);
        }
        self.schedule(ui, viz_wake);
    }

    /// Decides which visualization surface runs this frame — its panel surface
    /// while shown and not minimised, or its independent window (#303), which
    /// stays up even when the main window is minimised — records it on the
    /// shell, and returns the wake to schedule. A surface that just became
    /// active gets the frame-rate wake the shell did not yet know about (#305).
    fn update_viz_active(&mut self, ui: &Ui<Msg>, next_wake: Option<Duration>) -> Option<Duration> {
        let minimized = ui.placement().show == ShowState::Minimized;
        let surface = self.shell.state.projectm.surface();
        self.viz_panel_active =
            surface == Some(VizSurface::Panel) && self.shell.state.panels.right_panel && !minimized;
        self.viz_window_active = surface == Some(VizSurface::Window) && !self.viz_window_failed;
        self.viz_active = self.viz_panel_active || self.viz_window_active;
        self.shell.state.projectm.running = self.viz_active;
        if self.viz_active {
            Some(next_wake.map_or(FRAME_INTERVAL, |wait| wait.min(FRAME_INTERVAL)))
        } else {
            next_wake
        }
    }

    /// Opens the tag editor modal for the track the shell just resolved (via
    /// [`Command::OpenTagEditor`]), if none is open yet. Blocks until the
    /// dialog closes, then ends the session.
    fn open_tag_editor(&mut self, ui: &Ui<Msg>) {
        if self.tag_editor.is_some() {
            return;
        }
        let Some(state) = &self.shell.state.tag_editor else {
            return;
        };
        let bridge = std::rc::Rc::new(std::cell::RefCell::new(tag_editor::Bridge::new(
            state.status.clone(),
        )));
        self.tag_editor = Some(std::rc::Rc::clone(&bridge));
        tag_editor::show(ui, state, bridge, ui.proxy());
        self.shell.state.tag_editor = None;
        self.tag_editor = None;
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
                    | View::NowPlaying
                    | View::Visualization
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
            self.preset_browser.set_visible(view == View::Visualization);
            self.now_playing_central
                .set_visible(view == View::NowPlaying);
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
                changes,
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

        self.refresh_now_playing();

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
            let state = &self.shell.state;
            top_bar.sync(
                &state.top_bar,
                &state.search_query,
                state.visualizer_enabled,
            );
            if state.visualizer_enabled {
                top_bar.feed_visualizer(state.visualizer, self.shell.player.as_ref());
            }
        }

        // Panel visibility is toggled through `Command::TogglePanel`; apply it
        // and relayout only when it actually changed.
        let panels = self.shell.state.panels;
        if panels != self.applied_panels {
            self.navigator.set_visible(panels.navigator);
            self.right_panel.set_visible(panels.right_panel);
            self.right_panel
                .set_queue_visible(panels.right_panel && panels.next_tracks);
            self.status.set_visible(panels.status_bar);
            ui.relayout();
            self.applied_panels = panels;
            // Rebuild the View menu so its ticks match the new visibility
            // (win32ui has no checked-setter).
            ui.set_menu_bar(menu::build(&self.shell.state));
        }
        // The Visualization submenu's ticks change in place, so the bar is not
        // rebuilt when only a preset or placement changed (#306).
        self.sync_viz_menu(ui);

        // The panel's projectM row expands only while the shell shows its panel
        // surface; relayout when that changes. Otherwise it collapses to zero
        // height and the widget is hidden, which also stops it (#302, #305).
        let viz_visible = self.shell.state.projectm.surface() == Some(VizSurface::Panel)
            && self.shell.state.panels.right_panel;
        if viz_visible != self.applied_viz_visible {
            self.right_panel.set_viz_visible(viz_visible);
            self.applied_viz_visible = viz_visible;
            self.install_layout(ui, view);
        }
        // The row can stay shown while the app is minimised; only the animation
        // stops then, so a restore resumes without a relayout (#305). The
        // independent window is driven separately and keeps running (#303).
        self.right_panel.viz().set_running(self.viz_panel_active);
        self.sync_viz_window(ui);
        self.sync_projectm();
        self.sync_viz_lifecycle();
        // The preset browser reads the scanned list, so it syncs after the
        // lifecycle (which may have just installed a finished scan) (#338).
        if view == View::Visualization {
            self.preset_browser.sync(
                &self.shell.state.projectm.settings,
                self.preset_files.as_ref(),
                self.preset_scan_generation,
            );
        }

        // The dark/light toggle and the accent picker change the shell.s look;
        // mirror them onto the window.
        let look = (self.shell.state.theme, self.shell.state.accent);
        if look != self.applied_look {
            let theme = win32_theme(look.0, look.1);
            ui.set_theme(theme);
            if let Some(window) = &self.viz_window {
                window.set_theme(theme);
            }
            self.applied_look = look;
        }

        // Font size, density and DPI (#309): recompute the metrics once and
        // push them into every list and custom widget. Row heights are whole
        // device pixels, so the lists never draw fractional rows mid-change.
        let appearance = self.shell.state.appearance;
        let dpi = ui.dpi();
        if appearance != self.applied_appearance || dpi != self.applied_dpi {
            crate::appearance::install(appearance, dpi);
            self.apply_appearance(ui);
            self.applied_appearance = appearance;
            self.applied_dpi = dpi;
        }
    }

    /// Pushes the current appearance metrics and zebra flag into every view:
    /// the lists (row font, row height, striping) and the custom widgets
    /// (navigator, summary, album tiles) (#309).
    fn apply_appearance(&mut self, ui: &Ui<Msg>) {
        self.navigator.apply_appearance(ui);
        self.music.apply_appearance();
        self.albums.apply_appearance();
        self.folders.apply_appearance();
        self.artists.apply_appearance();
        self.genres.apply_appearance();
        self.most_played.apply_appearance();
        self.history.apply_appearance();
        self.preset_browser.apply_appearance();
        self.starred.apply_appearance();
        self.browser.apply_appearance();
        self.right_panel.apply_appearance();
        self.now_playing_central.apply_appearance();
        self.settings.apply_appearance();
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

    /// Keeps whichever projectM surface is active in step with the shell:
    /// settings, audio, preset requests and its events. The panel reads the
    /// player directly; the independent window (#303) is fed PCM and preset
    /// files the main app owns. While a surface is inactive it is neither fed
    /// nor has its stale requests replayed (#302, #305).
    fn sync_projectm(&mut self) {
        let panel_active = self.viz_panel_active;

        if panel_active {
            self.right_panel
                .viz()
                .set_settings(&self.shell.state.projectm.settings);
            self.right_panel
                .viz()
                .set_last_preset(self.shell.state.projectm.settings.last_preset.as_deref());
            self.right_panel.viz().feed(self.shell.player.as_ref());
        }

        if self.viz_window_active
            && let Some(window) = &self.viz_window
        {
            window.set_settings(&self.shell.state.projectm.settings);
            window.set_last_preset(self.shell.state.projectm.settings.last_preset.as_deref());
            if let Some(files) = &self.preset_files {
                window.set_presets(files.clone());
            }
            let player = self.shell.player.as_ref();
            if player.status() == PlaybackStatus::Playing {
                window.feed_samples(&player.samples());
            } else {
                window.push_silence();
            }
        }

        for request in self.shell.state.projectm.take_requests() {
            if panel_active {
                self.right_panel.viz().request_preset(request);
            } else if self.viz_window_active
                && let Some(window) = &self.viz_window
            {
                window.request_preset(request);
            }
        }

        // Events and the reported availability belong to the panel; the
        // independent window forwards both to the main queue itself (#303).
        if panel_active {
            for event in self.right_panel.viz().take_events() {
                match event {
                    ProjectMEvent::PresetShown(path) => {
                        self.shell.state.projectm.settings.last_preset = Some(path);
                    }
                    ProjectMEvent::AvailabilityChanged(availability) => {
                        self.shell.state.projectm.availability = availability;
                    }
                }
            }
            let availability = self.right_panel.viz().availability();
            if self.shell.state.projectm.availability != availability {
                self.shell.state.projectm.availability = availability;
            }
        } else {
            let _ = self.right_panel.viz().take_events();
        }
    }

    /// Opens the independent visualization window (#303) when the shell points
    /// the surface at it, shows it again after a hide, and hides it (keeping
    /// its state) when the surface moves elsewhere.
    fn sync_viz_window(&mut self, ui: &Ui<Msg>) {
        if let Some(window) = &self.viz_window
            && !window.is_alive()
        {
            self.viz_window = None;
        }
        if self.viz_window_active {
            if self.viz_window.is_none() && !self.viz_window_failed {
                match VizWindow::open(ui, self.shell.state.viz_window) {
                    Ok(window) => {
                        tracing::info!("opened the projectM visualization window");
                        self.viz_window = Some(window);
                    }
                    Err(err) => {
                        tracing::warn!(%err, "could not open the visualization window");
                        self.viz_window_failed = true;
                    }
                }
            }
            if let Some(window) = &self.viz_window
                && !window.is_visible()
            {
                window.show();
            }
        } else if let Some(window) = &self.viz_window
            && window.is_visible()
        {
            window.hide();
        }
    }

    /// Records the visualization window's live geometry for the next launch
    /// (#303), so it reopens where the user left it.
    fn record_viz_geometry(&mut self) {
        if let Some(window) = &self.viz_window {
            self.shell.state.viz_window = window.geometry();
        }
    }

    /// Updates the View → Visualization menu ticks from the shell state, in
    /// place, so the bar is only rebuilt when the panel/browser layout does
    /// (#306).
    fn sync_viz_menu(&mut self, ui: &Ui<Msg>) {
        let ticks = viz_menu_ticks(&self.shell.state.projectm);
        if ticks == self.applied_viz_menu {
            return;
        }
        self.applied_viz_menu = ticks;
        let (show, locked, panel, window, fullscreen) = ticks;
        ui.set_menu_checked("viz-show", show);
        ui.set_menu_checked("viz-lock", locked);
        ui.set_menu_checked("viz-panel", panel);
        ui.set_menu_checked("viz-window", window);
        ui.set_menu_checked("viz-fullscreen", fullscreen);
    }

    /// Runs the visualization's stop/lifecycle policy: arm or cancel the grace
    /// period, free the instance once it expires, and keep the background
    /// preset scan in step (#305).
    fn sync_viz_lifecycle(&mut self) {
        let now = Instant::now();
        if self.viz_active {
            self.viz_grace.disarm();
        } else {
            self.viz_grace.arm(now);
            if self.viz_grace.take_due(now) {
                tracing::debug!("projectM idle grace expired; freeing the instance");
                // Either surface may hold the instance (both are hidden while
                // inactive); both frees are no-ops when it is not there (#303).
                self.right_panel.viz().suspend();
                if let Some(window) = &self.viz_window {
                    window.suspend();
                }
            }
        }
        self.poll_preset_scan();
    }

    /// Starts a background preset scan when the preset configuration changed,
    /// and hands a finished scan to the surface (#305).
    fn poll_preset_scan(&mut self) {
        let config = {
            let settings = &self.shell.state.projectm.settings;
            (
                settings.disabled_packs.clone(),
                settings.user_preset_dir.clone(),
            )
        };
        if self.preset_config.as_ref() != Some(&config) {
            self.preset_config = Some(config);
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(PathBuf::from))
                .unwrap_or_default();
            let roots = PresetRoots::resolve(&exe_dir, &self.shell.state.projectm.settings);
            tracing::debug!(?roots, "scanning projectM presets");
            self.preset_scanner = Some(PresetScanner::spawn(roots));
        }
        if let Some(files) = self
            .preset_scanner
            .as_ref()
            .and_then(PresetScanner::try_take)
        {
            tracing::info!(
                presets = files.presets.len(),
                textures = files.textures.len(),
                "projectM preset scan ready"
            );
            self.right_panel.viz().set_presets(files.clone());
            if let Some(window) = &self.viz_window {
                window.set_presets(files.clone());
            }
            self.preset_files = Some(files);
            self.preset_scan_generation += 1;
            self.preset_scanner = None;
        }
    }

    /// Serves a deterministic placeholder preset list instead of a real scan,
    /// for `emusic-shot` and `emusic --mock` (#338). Cancels the running scan
    /// and records the current preset configuration so [`Self::poll_preset_scan`]
    /// does not immediately replace it.
    pub fn seed_placeholder_presets(&mut self) {
        let settings = &self.shell.state.projectm.settings;
        self.preset_config = Some((
            settings.disabled_packs.clone(),
            settings.user_preset_dir.clone(),
        ));
        self.preset_scanner = None;
        let files = PresetFiles::placeholder();
        self.right_panel.viz().set_presets(files.clone());
        if let Some(window) = &self.viz_window {
            window.set_presets(files.clone());
        }
        self.preset_files = Some(files);
        self.preset_scan_generation += 1;
    }

    /// Refreshes the now-playing model from the player/library, then pushes it
    /// into the panel (artwork included).
    fn refresh_now_playing(&mut self) {
        let shell = &mut self.shell;
        shell
            .state
            .now_playing
            .refresh(shell.player.as_ref(), shell.library.as_ref());
        self.right_panel.sync(&shell.state.now_playing);
        // The central view is only shown for `View::NowPlaying`, but it is
        // always synced: it is cheap (`sync` itself skips work the model's
        // revision didn't change) and keeps it ready the instant the view
        // becomes visible.
        self.now_playing_central.sync(&shell.state.now_playing);
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

    /// Keeps a single repeating timer in step with the shell's `next_wake` and
    /// the projectM stop grace period, so a stopped instance is freed even
    /// while the shell is otherwise idle (#305).
    fn schedule(&mut self, ui: &mut Ui<Msg>, next_wake: Option<std::time::Duration>) {
        let mut wanted = next_wake;
        if let Some(grace) = self.viz_grace.wait(Instant::now()) {
            wanted = Some(wanted.map_or(grace, |current| current.min(grace)));
        }
        let wanted = wanted.map(|wait| wait.as_millis().min(u128::from(u32::MAX)) as u32);
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
            Msg::Wake => self.tick(ui),
            Msg::Timer => self.on_timer(ui),
            Msg::Dispatch(command) => {
                self.shell.dispatch(command);
                self.tick(ui);
            }
            Msg::DatabaseInfo => {
                let choice = database_info::show(ui, self.shell.library.as_ref());
                if choice == Some(DatabaseInfoChoice::Rescan) {
                    self.shell.dispatch(Command::LibraryRescan);
                    self.tick(ui);
                }
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
                // Properties opens a modal dialog, not a playback command.
                if action == ContextAction::Properties {
                    let track = match self.shell.state.view {
                        View::Music => self.music.context_track(),
                        View::Folders => self.folders.context_track(),
                        View::Starred => self.starred.context_track(),
                        View::MostPlayed => self.most_played.context_track(),
                        View::History => self.history.context_track_id().and_then(|id| {
                            self.shell
                                .library
                                .tracks()
                                .iter()
                                .find(|track| track.id == id)
                                .cloned()
                        }),
                        _ => None,
                    };
                    if let Some(track) = track {
                        properties::show(ui, &track);
                    }
                    return;
                }
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
                self.open_tag_editor(ui);
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
            // The splitter already moved itself live; only the persisted width
            // is recorded here (#342). The next layout install seeds from it.
            Msg::BrowserSplit(position) => {
                self.shell.state.music.browser.height = position
                    .value()
                    .clamp(BROWSER_MIN_HEIGHT, BROWSER_MAX_HEIGHT);
            }
            Msg::NavigatorSplit(position) => {
                self.shell.state.navigator_width = position
                    .value()
                    .clamp(MIN_NAVIGATOR_WIDTH, MAX_NAVIGATOR_WIDTH);
            }
            Msg::RightPanelSplit(position) => {
                self.shell.state.right_panel_width = position
                    .value()
                    .clamp(MIN_RIGHT_PANEL_WIDTH, MAX_RIGHT_PANEL_WIDTH);
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
            Msg::NavigatorContext(view) => {
                if view != View::Music {
                    return;
                }
                let menu = Menu::new().item("Shuffle all", None, || Msg::NavigatorShuffleAll);
                ui.popup(&menu, ui.cursor_position());
            }
            Msg::NavigatorShuffleAll => {
                let ids: Vec<u64> = self
                    .shell
                    .library
                    .tracks()
                    .iter()
                    .map(|track| track.id)
                    .collect();
                self.shell.dispatch(Command::ShuffleScope {
                    ids,
                    label: "All tracks".to_string(),
                });
                self.tick(ui);
            }
            Msg::MusicShuffleAll => {
                let command = self.music.shuffle_all(
                    &self.shell.state,
                    self.shell.library.as_ref(),
                    &self.shell.search,
                );
                self.shell.dispatch(command);
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
                // The shared model parks the track in `properties`; the
                // native dialog is modal, so show it right away and clear
                // the slot.
                if let Some(track) = self.shell.state.now_playing.properties.take() {
                    properties::show(ui, &track);
                }
                self.tick(ui);
                self.open_tag_editor(ui);
            }
            Msg::TagEditorApply => {
                let request = self
                    .tag_editor
                    .as_ref()
                    .and_then(|bridge| bridge.borrow_mut().apply.take());
                if let Some(request) = request {
                    if let Some(editor) = self.shell.state.tag_editor.as_mut() {
                        editor.status = emusic_ui::tag_editor::Status::Pending;
                    }
                    self.shell.dispatch(Command::RequestTagEdits(vec![request]));
                    self.tick(ui);
                }
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
            Msg::CentralQueueJump(row) => {
                if let Some(index) = self.now_playing_central.queue_index(row) {
                    self.apply_now_playing(NowPlayingMsg::QueueJump(index));
                    self.tick(ui);
                }
            }
            Msg::CentralQueueContext(row) => {
                self.now_playing_central.set_context_row(row);
                ui.popup(
                    self.now_playing_central.context_menu(),
                    ui.cursor_position(),
                );
            }
            Msg::CentralQueueRemove => {
                if let Some(index) = self.now_playing_central.context_index() {
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
            Msg::CycleVisualizer => {
                self.shell.dispatch(Command::CycleVisualizer);
                self.tick(ui);
            }
            Msg::Viz(command) => {
                self.shell.dispatch(Command::Viz(command));
                self.tick(ui);
            }
            Msg::VizAvailability(availability) => {
                self.shell.state.projectm.availability = availability;
            }
            Msg::VizMenu => {
                let menu = menu::viz_context(&self.shell.state);
                ui.popup(&menu, ui.cursor_position());
            }
            Msg::PresetFilter(filter) => {
                self.preset_browser.set_filter(&filter);
                self.tick(ui);
            }
            Msg::PresetSelect(row) => {
                self.preset_browser.select(row);
                self.tick(ui);
            }
            Msg::PresetPlay(row) => {
                if let Some(index) = self.preset_browser.playlist_index(row) {
                    for command in preset_browser::play_commands(index) {
                        self.shell.dispatch(command);
                    }
                    self.tick(ui);
                }
            }
            Msg::Quit => {
                self.shell.save_on_exit();
                ui.close();
            }
        }
    }
}

/// Applies the saved window geometry to the freshly created window, before it
/// is first shown (#214), overriding the default centring. The saved values are
/// logical points; the window's placement is in device pixels.
fn apply_saved_geometry(ui: &Ui<Msg>, saved: WindowGeometry) {
    if saved.size.is_none() && saved.position.is_none() && !saved.maximized {
        return;
    }
    let scale = dpi_scale(ui.dpi());
    let current = ui.window_rect();
    // The saved size is the client area (the inner rect); a placement's normal
    // bounds are the outer rectangle, so add the
    // frame the window was just created with.
    let outer = current.size();
    let client = ui.client_rect().size();
    let frame_w = (outer.width - client.width).max(0);
    let frame_h = (outer.height - client.height).max(0);
    let width = saved
        .size
        .map_or(current.width(), |[w, _]| to_px(w, scale) + frame_w);
    let height = saved
        .size
        .map_or(current.height(), |[_, h]| to_px(h, scale) + frame_h);
    let left = saved
        .position
        .map_or(current.left, |[x, _]| to_px(x, scale));
    let top = saved.position.map_or(current.top, |[_, y]| to_px(y, scale));
    let normal = Rect::new(left, top, left + width, top + height);
    let show = if saved.maximized {
        ShowState::Maximized
    } else {
        ShowState::Normal
    };
    let placement = Placement { normal, show }.clamp_to_work_areas();
    let _ = ui.set_placement(&placement);
}

/// Records the live window geometry into the shared state, so the config
/// written on exit restores it next launch (#214).
/// A maximized window records only the flag, keeping the last normal geometry.
fn record_window_geometry(ui: &Ui<Msg>, state: &mut AppState) {
    let placement = ui.placement();
    state.window.maximized = placement.show == ShowState::Maximized;
    if state.window.maximized {
        return;
    }
    let scale = 1.0 / dpi_scale(ui.dpi());
    let normal = placement.normal;
    let client = ui.client_rect().size();
    state.window.size = Some([client.width as f32 * scale, client.height as f32 * scale]);
    state.window.position = Some([normal.left as f32 * scale, normal.top as f32 * scale]);
}

/// Device pixels per logical point at `dpi` (96 DPI is 1:1).
fn dpi_scale(dpi: u32) -> f32 {
    dpi as f32 / 96.0
}

/// A logical-point value converted to device pixels.
fn to_px(value: f32, scale: f32) -> i32 {
    (value * scale).round() as i32
}

/// Reveals `path` in Explorer.
fn open_file_location(path: &str) {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.replace('/', "\\")))
        .spawn();
}

/// The Visualization menu ticks for a projectM state: shown, locked, panel,
/// window, fullscreen (#306).
fn viz_menu_ticks(projectm: &emusic_ui::state::ProjectMState) -> (bool, bool, bool, bool, bool) {
    let layout = &projectm.layout;
    (
        layout.visible,
        projectm.settings.preset_locked,
        !layout.fullscreen && layout.dock == VizDock::Panel,
        !layout.fullscreen && layout.dock == VizDock::Window,
        layout.fullscreen,
    )
}

/// The library id of the player's current track, matched by path.
fn playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library
        .track_by_path(&now_playing.path)
        .map(|track| track.id)
}
