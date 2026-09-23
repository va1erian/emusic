//! The [`eframe::App`] implementation: owns the app's state and backends,
//! wires the menu bar, panels and central view router together, applies
//! queued [`Command`]s and implements the repaint policy from #6 (no
//! continuous repaint; ~30 fps only while something is actually playing).
//!
//! The pieces are split by responsibility: [`update`] is the per-frame
//! `eframe::App` loop and config persistence, [`events`] handles input and
//! messages from other instances, and [`commands`] translates queued
//! [`Command`]s into player/library calls.
//!
//! [`Command`]: crate::state::Command

mod commands;
mod events;
mod update;

use std::path::PathBuf;

use crate::config::{self, Config};
use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::search::SearchEngine;
use crate::state::{AppState, View};
use crate::{fonts, theme};

pub struct App {
    state: AppState,
    library: Box<dyn LibraryDataSource>,
    player: Box<dyn PlayerApi>,
    /// Where the config is persisted; `None` disables all disk I/O (used
    /// by `emusic-shot` and the snapshot tests for determinism).
    config_path: Option<PathBuf>,
    /// Config as last loaded/saved, compared each frame to detect changes.
    saved: Config,
    /// When the live settings first diverged from `saved`; drives the
    /// debounced save.
    dirty_since: Option<std::time::Instant>,
    /// Set when this process is the primary instance (#11); polled once per
    /// frame for messages a secondary launch forwarded.
    ipc: Option<crate::backend::ipc::IpcBridge>,
    /// A startup problem to keep showing the user (e.g. "no audio device")
    /// rather than silently degrading; `None` once nothing is wrong.
    backend_notice: Option<String>,
    /// Live full-text search over the library for the top bar / Music view
    /// (#22); owns a background worker so matching never blocks the UI
    /// thread.
    search: SearchEngine,
    /// A second, independent search engine for the global search popup, so
    /// its query never changes what the Music view underneath is showing.
    popup_search: SearchEngine,
    /// OS media controls / hardware media keys (#26). `None` in `emusic-shot`
    /// and the snapshot tests, so headless runs never touch SMTC.
    smtc: Option<crate::backend::smtc::Smtc>,
    /// Windows taskbar thumbnail-toolbar transport buttons (#42). `None` in
    /// `emusic-shot` and the snapshot tests so headless runs never touch the
    /// shell.
    thumbbar: Option<crate::backend::thumbbar::ThumbBar>,
}

impl App {
    /// Builds the app, restoring persisted settings from
    /// `%APPDATA%\emusic\config.toml` (defaults when absent or bad).
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
    ) -> Self {
        let path = config::config_path();
        let saved = path.as_deref().map_or_else(Config::default, config::load);
        Self::build(cc, library, player, saved, path)
    }

    /// Builds the app from an explicit [`Config`] with persistence
    /// disabled. Used by `emusic-shot` and the snapshot tests so renders
    /// stay deterministic and never touch the user's config.
    pub fn with_config(
        cc: &eframe::CreationContext<'_>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
        config: Config,
    ) -> Self {
        Self::build(cc, library, player, config, None)
    }

    /// Builds the app for this run. A normal launch ([`Self::new`]) restores
    /// and persists `%APPDATA%\emusic\config.toml`; a `--mock` launch (#135)
    /// instead starts from [`Config::default`] with persistence disabled
    /// ([`Self::with_config`]). The mock library's synthetic folders must
    /// never leak into the real user's config, and mock reads nothing from
    /// it either: `config::load` can rename an unparsable file to
    /// `config.toml.bak`, which a preview mode has no business doing.
    pub fn for_run(
        cc: &eframe::CreationContext<'_>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
        mock: bool,
    ) -> Self {
        if mock {
            Self::with_config(cc, library, player, Config::default())
        } else {
            Self::new(cc, library, player)
        }
    }

    fn build(
        cc: &eframe::CreationContext<'_>,
        mut library: Box<dyn LibraryDataSource>,
        mut player: Box<dyn PlayerApi>,
        config: Config,
        config_path: Option<PathBuf>,
    ) -> Self {
        fonts::install(&cc.egui_ctx);
        crate::settings::folder_picker::init();
        let mut state = AppState::default();
        config.apply_to_state(&mut state);
        theme::apply(&cc.egui_ctx, state.theme, state.accent.color());
        config.apply_to_player(player.as_mut());
        library.set_folders(&config.library_folders);
        Self {
            state,
            library,
            player,
            config_path,
            saved: config,
            dirty_since: None,
            ipc: None,
            backend_notice: None,
            search: SearchEngine::new(),
            popup_search: SearchEngine::new(),
            smtc: None,
            thumbbar: None,
        }
    }

    /// Registers this process's [`IpcBridge`] (present only for the primary
    /// instance, #11); polled once per frame in [`App::ui`].
    ///
    /// [`IpcBridge`]: crate::backend::ipc::IpcBridge
    /// [`App::ui`]: eframe::App::ui
    pub fn attach_ipc(&mut self, ipc: crate::backend::ipc::IpcBridge) {
        self.ipc = Some(ipc);
    }

    /// Registers the OS media-control integration (#26). Only the real binary
    /// calls this; shot/tests leave it unset so they never touch SMTC.
    ///
    /// [`Smtc`]: crate::backend::smtc::Smtc
    pub fn attach_smtc(&mut self, smtc: crate::backend::smtc::Smtc) {
        self.smtc = Some(smtc);
    }

    /// Registers the Windows taskbar thumbnail-toolbar buttons (#42). Only
    /// the real binary calls this; shot/tests leave it unset so they never
    /// touch the shell.
    ///
    /// [`ThumbBar`]: crate::backend::thumbbar::ThumbBar
    pub fn attach_thumbbar(&mut self, thumbbar: crate::backend::thumbbar::ThumbBar) {
        self.thumbbar = Some(thumbbar);
    }

    /// Sets a one-line startup notice (e.g. "Audio unavailable: ...") shown
    /// under the menu bar until the app is restarted.
    pub fn set_backend_notice(&mut self, notice: impl Into<String>) {
        self.backend_notice = Some(notice.into());
    }

    /// Jumps straight to a view, bypassing the navigator click. Used by
    /// `emusic-shot` so every view (including ones with no navigator entry,
    /// like Settings) can be screenshotted directly.
    pub fn set_view(&mut self, view: View) {
        self.state.view = view;
    }

    /// Selects a Settings sub-page (#137), bypassing the tab click. Used by
    /// `emusic-shot` (`--settings-tab`) so each sub-page can be
    /// screenshotted directly.
    pub fn set_settings_tab(&mut self, tab: crate::state::SettingsTab) {
        self.state.settings_tab = tab;
    }

    /// Sets the top-bar search box's query text directly, bypassing the
    /// widget. Used by `emusic-shot` (`--query`) so a filtered Music view
    /// can be screenshotted headlessly.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.state.search_query = query.into();
    }

    /// Opens the global search popup with the given query, as Ctrl+K would.
    /// Used by `emusic-shot` (`--search-popup`).
    pub fn open_search_popup(&mut self, query: impl Into<String>) {
        self.state.search_popup.open();
        self.state.search_popup.query = query.into();
    }

    /// Opens the Music table's track Properties dialog for the library's
    /// first track, as the row's context menu would (#136). Used by
    /// `emusic-shot` (`--properties`) so the dialog can be screenshotted
    /// headlessly.
    pub fn open_track_properties(&mut self) {
        if let Some(track) = self.library.tracks().first() {
            self.state.music_table.properties = Some(track.clone());
        }
    }
}
