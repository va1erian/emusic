//! The [`eframe::App`] implementation: a thin egui frontend over the
//! toolkit-agnostic [`Shell`] (whose `tick` owns polling, command
//! application and config persistence, #97). This type only installs the
//! fonts/theme, draws [`Shell::state`] with egui, and maps the shell's
//! `next_wake` onto egui's repaint scheduling.
//!
//! The pieces are split by responsibility: [`update`] is the per-frame
//! `eframe::App` loop, and [`events`] handles egui input and draws the menu
//! bar.

mod events;
pub(crate) mod images;
mod update;

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui;

use crate::config::{self, Config};
use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::shell::Shell;
use crate::state::{Accent, Appearance, Theme, View};
use crate::waker::WakerSlot;
use crate::{fonts, theme};

use self::images::ImageCaches;

/// The egui frontend.
pub struct EguiApp {
    shell: Shell,
    /// egui-bound image caches (album thumbnails, now-playing artwork, #96).
    images: ImageCaches,
    /// OS media controls / hardware media keys (#26). `None` in `emusic-shot`
    /// and the snapshot tests, so headless runs never touch SMTC.
    smtc: Option<crate::backend::smtc::Smtc>,
    /// Windows taskbar thumbnail-toolbar transport buttons (#42). `None` in
    /// `emusic-shot` and the snapshot tests so headless runs never touch the
    /// shell.
    thumbbar: Option<crate::backend::thumbbar::ThumbBar>,
    /// A deterministic clock advanced by egui's frame delta (not wall-clock
    /// time), so headless renders (#32) stay reproducible. Only differences
    /// matter to [`Shell::tick`].
    synthetic_now: Instant,
    /// The theme/accent/appearance actually applied to egui, so the style is
    /// rebuilt on change (not every frame) and a DPI move re-applies it.
    applied: Option<AppliedAppearance>,
}

/// The inputs [`theme::apply`] was last called with, for change detection.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AppliedAppearance {
    theme: Theme,
    accent: Accent,
    appearance: Appearance,
    /// `pixels_per_point().to_bits()`, so the float is compared exactly.
    dpi_bits: u32,
}

impl EguiApp {
    /// Builds the app from an explicit [`Config`] with persistence
    /// disabled. Used by `emusic-shot` and the snapshot tests so renders
    /// stay deterministic and never touch the user's config.
    pub fn with_config(
        cc: &eframe::CreationContext<'_>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
        config: Config,
    ) -> Self {
        Self::build(cc, library, player, config, None, WakerSlot::new())
    }

    /// Builds the app for this run. A normal launch restores and persists
    /// `%APPDATA%\emusic\config.toml`; a `--mock` launch (#135) instead
    /// starts from [`Config::default`] with persistence disabled. The mock
    /// library's synthetic folders must never leak into the real user's
    /// config, and mock reads nothing from it either: `config::load` can
    /// rename an unparsable file to `config.toml.bak`, which a preview mode
    /// has no business doing.
    ///
    /// `waker` wakes the UI from background work (the search workers); a
    /// `--mock` run gets an unbound slot because it does not persist.
    pub fn for_run(
        cc: &eframe::CreationContext<'_>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
        mock: bool,
        waker: WakerSlot,
    ) -> Self {
        if mock {
            Self::build(cc, library, player, Config::default(), None, waker)
        } else {
            let path = config::config_path();
            let saved = path.as_deref().map_or_else(Config::default, config::load);
            Self::build(cc, library, player, saved, path, waker)
        }
    }

    fn build(
        cc: &eframe::CreationContext<'_>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
        config: Config,
        config_path: Option<PathBuf>,
        waker: WakerSlot,
    ) -> Self {
        fonts::install(&cc.egui_ctx);
        crate::settings::folder_picker::init();
        let images = ImageCaches::new(waker.handle());
        let shell = Shell::new(library, player, config, config_path, waker);
        theme::apply(
            &cc.egui_ctx,
            shell.state.theme,
            shell.state.accent,
            shell.state.appearance,
        );
        let applied = AppliedAppearance {
            theme: shell.state.theme,
            accent: shell.state.accent,
            appearance: shell.state.appearance,
            dpi_bits: cc.egui_ctx.pixels_per_point().to_bits(),
        };
        Self {
            shell,
            images,
            smtc: None,
            thumbbar: None,
            synthetic_now: Instant::now(),
            applied: Some(applied),
        }
    }

    /// Re-applies the theme when the theme, accent, appearance or DPI changed
    /// since the last frame, so text sizes and row metrics stay in sync with
    /// the Settings choices without rebuilding egui's style every frame.
    pub(crate) fn sync_appearance(&mut self, ctx: &egui::Context) {
        let state = &self.shell.state;
        let applied = AppliedAppearance {
            theme: state.theme,
            accent: state.accent,
            appearance: state.appearance,
            dpi_bits: ctx.pixels_per_point().to_bits(),
        };
        if self.applied == Some(applied) {
            return;
        }
        theme::apply(ctx, applied.theme, applied.accent, applied.appearance);
        self.applied = Some(applied);
    }

    /// The shell this frontend drives, for tests and the screenshot tool.
    pub fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }

    /// Registers this process's [`IpcBridge`] (present only for the primary
    /// instance, #11); polled once per tick by the shell.
    ///
    /// [`IpcBridge`]: crate::backend::ipc::IpcBridge
    pub fn attach_ipc(&mut self, ipc: crate::backend::ipc::IpcBridge) {
        self.shell.attach_ipc(ipc);
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
        self.shell.set_backend_notice(notice);
    }

    /// Applies a request from the CLI or from a secondary instance (#11).
    pub fn handle_ipc_message(&mut self, message: winshell::IpcMessage) {
        self.shell.handle_ipc_message(message);
    }

    /// Jumps straight to a view, bypassing the navigator click. Used by
    /// `emusic-shot` so every view (including ones with no navigator entry,
    /// like Settings) can be screenshotted directly.
    pub fn set_view(&mut self, view: View) {
        self.shell.state.view = view;
    }

    /// Selects a Settings sub-page (#137), bypassing the tab click. Used by
    /// `emusic-shot` (`--settings-tab`) so each sub-page can be
    /// screenshotted directly.
    pub fn set_settings_tab(&mut self, tab: crate::state::SettingsTab) {
        self.shell.state.settings_tab = tab;
    }

    /// Sets the top-bar search box's query text directly, bypassing the
    /// widget. Used by `emusic-shot` (`--query`) so a filtered Music view
    /// can be screenshotted headlessly.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.shell.state.search_query = query.into();
    }

    /// Opens the global search popup with the given query, as Ctrl+K would.
    /// Used by `emusic-shot` (`--search-popup`).
    pub fn open_search_popup(&mut self, query: impl Into<String>) {
        self.shell.state.search_popup.state.open();
        self.shell.state.search_popup.state.query = query.into();
    }

    /// Opens the Music table's track Properties dialog for the library's
    /// first track, as the row's context menu would (#136). Used by
    /// `emusic-shot` (`--properties`) so the dialog can be screenshotted
    /// headlessly.
    pub fn open_track_properties(&mut self) {
        if let Some(track) = self.shell.library.tracks().first() {
            self.shell.state.music.table.properties = Some(track.clone());
        }
    }

    /// Opens the File -> Database info dialog. Used by `emusic-shot`
    /// (`--database-info`) so the dialog can be screenshotted headlessly.
    pub fn open_database_info(&mut self) {
        self.shell.state.database_info_open = true;
    }

    /// Opens the single-track tag editor for the library's first track, as
    /// the row's context menu would (#172). Used by `emusic-shot`
    /// (`--tag-editor`) so the dialog can be screenshotted headlessly.
    pub fn open_tag_editor(&mut self) {
        if let Some(track) = self.shell.library.tracks().first() {
            self.shell.state.tag_editor = Some(crate::tag_editor::TagEditorState::new(track));
        }
    }

    /// Sets the open tag editor's auto-tag lookup state, so `emusic-shot`
    /// (`--tag-editor-state`) and the snapshot tests can render the
    /// Searching / Matches / No-match states (#209) without a live lookup.
    pub fn set_tag_editor_auto_tag(&mut self, state: crate::tag_editor::AutoTagState) {
        if let Some(editor) = self.shell.state.tag_editor.as_mut() {
            editor.auto_tag = state;
        }
    }
}
