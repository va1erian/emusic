//! The [`eframe::App`] implementation: wires the menu bar, the four fixed
//! panels and the central view router together, applies queued
//! [`Command`]s (see [`commands`]), and implements the repaint policy from
//! #6 (no continuous repaint; ~30 fps only while something is actually
//! playing).

mod commands;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use tracing::warn;

use crate::backend::ipc::IpcBridge;
use crate::config::{self, Config};
use crate::library_api::LibraryDataSource;
use crate::player_api::{PlaybackStatus, PlayerApi};
use crate::state::{AppState, Command, PanelKind, View};
use crate::{fonts, panels, theme, views};

/// Cap on repaint rate while playing, per #6 ("`request_repaint_after(33ms)`
/// only while playing and window not minimized").
const PLAYING_REPAINT_INTERVAL: Duration = Duration::from_millis(33);

/// How long after a settings change the config is written, per #8; further
/// changes within the window restart the countdown.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(2);

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
    dirty_since: Option<Instant>,
    /// Set when this process is the primary instance (#11); polled once per
    /// frame for messages a secondary launch forwarded.
    ipc: Option<IpcBridge>,
    /// A startup problem to keep showing the user (e.g. "no audio device")
    /// rather than silently degrading; `None` once nothing is wrong.
    backend_notice: Option<String>,
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

    fn build(
        cc: &eframe::CreationContext<'_>,
        mut library: Box<dyn LibraryDataSource>,
        mut player: Box<dyn PlayerApi>,
        config: Config,
        config_path: Option<PathBuf>,
    ) -> Self {
        fonts::install(&cc.egui_ctx);
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
        }
    }

    /// Registers this process's [`IpcBridge`] (present only for the primary
    /// instance, #11); polled once per frame in [`App::ui`].
    pub fn attach_ipc(&mut self, ipc: IpcBridge) {
        self.ipc = Some(ipc);
    }

    /// Sets a one-line startup notice (e.g. "Audio unavailable: ...") shown
    /// under the menu bar until the app is restarted.
    pub fn set_backend_notice(&mut self, notice: impl Into<String>) {
        self.backend_notice = Some(notice.into());
    }

    /// Applies a request from the CLI or from a secondary instance (#11):
    /// stop and replace the queue with `message`'s files (or append them),
    /// resolved against its working directory.
    pub fn handle_ipc_message(&mut self, message: winshell::IpcMessage) {
        let paths = crate::backend::ipc::resolve_paths(&message);
        if paths.is_empty() {
            return;
        }
        tracing::info!(
            count = paths.len(),
            enqueue = message.enqueue,
            "handling IPC message"
        );
        if message.enqueue {
            for path in &paths {
                self.player.enqueue(path);
            }
        } else {
            self.player.replace_and_play(&paths, 0);
        }
    }

    /// Jumps straight to a view, bypassing the navigator click. Used by
    /// `emusic-shot` so every view (including ones with no navigator entry,
    /// like Settings) can be screenshotted directly.
    pub fn set_view(&mut self, view: View) {
        self.state.view = view;
    }

    fn apply_pending(&mut self) {
        let commands = std::mem::take(&mut self.state.pending);
        for cmd in &commands {
            self.state.apply_local(cmd);
            commands::apply_player_command(self.player.as_mut(), self.library.as_ref(), cmd);
        }
    }

    /// Config persistence policy from #8: write 2 s after the last change
    /// (plus once more on exit via [`App::on_exit`]). No-op when
    /// persistence is disabled.
    fn tick_config_persistence(&mut self) {
        if self.config_path.is_none() {
            return;
        }
        let current = Config::capture(&self.state, self.player.as_ref());
        if current == self.saved {
            self.dirty_since = None;
            return;
        }
        let dirty_since = *self.dirty_since.get_or_insert(Instant::now());
        if dirty_since.elapsed() >= SAVE_DEBOUNCE {
            self.write_config(current);
        }
    }

    fn write_config(&mut self, config: Config) {
        let Some(path) = self.config_path.clone() else {
            return;
        };
        match config::save(&path, &config) {
            Ok(()) => {
                self.saved = config;
                self.dirty_since = None;
            }
            Err(err) => {
                warn!(%err, path = %path.display(), "could not save config");
            }
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::containers::menu::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("View", |ui| {
                    self.panel_menu_item(ui, "Navigator", PanelKind::Navigator);
                    self.panel_menu_item(ui, "Now playing panel", PanelKind::RightPanel);
                    self.panel_menu_item(ui, "Status bar", PanelKind::StatusBar);
                    ui.separator();
                    if ui.button("Toggle dark / light theme").clicked() {
                        self.state.push(Command::ToggleTheme);
                        ui.close();
                    }
                });
                ui.menu_button("Window", |ui| {
                    for view in View::ALL {
                        if ui.button(view.label()).clicked() {
                            self.state.push(Command::SetView(view));
                            ui.close();
                        }
                    }
                });
            });
        });
    }

    fn panel_menu_item(&mut self, ui: &mut egui::Ui, label: &str, kind: PanelKind) {
        let visible = match kind {
            PanelKind::Navigator => self.state.panels.navigator,
            PanelKind::RightPanel => self.state.panels.right_panel,
            PanelKind::StatusBar => self.state.panels.status_bar,
        };
        let mut checked = visible;
        if ui.checkbox(&mut checked, label).changed() {
            self.state.push(Command::TogglePanel(kind));
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Apply background updates (new library snapshots, scan progress,
        // recorded plays) before any view reads the data.
        self.library.tick();

        // Driven by egui's own (deterministic, harness-controllable) frame
        // delta rather than a wall-clock `Instant`, so headless renders
        // (#32) are reproducible instead of depending on real elapsed time.
        let dt = ctx.input(|i| i.stable_dt);
        self.player.tick(Duration::from_secs_f32(dt.max(0.0)));

        theme::apply(&ctx, self.state.theme, self.state.accent.color());

        if let Some(message) = self.ipc.as_ref().and_then(IpcBridge::try_recv) {
            self.handle_ipc_message(message);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        self.menu_bar(ui);
        if let Some(notice) = self.backend_notice.clone() {
            egui::Panel::top("backend_notice")
                .exact_size(24.0)
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.colored_label(ui.visuals().warn_fg_color, notice);
                    });
                });
        }
        panels::top_bar::show(ui, &mut self.state, self.player.as_ref());
        if self.state.panels.status_bar {
            panels::status_bar::show(ui, self.library.as_ref(), self.player.as_ref());
        }
        if self.state.panels.navigator {
            panels::navigator::show(ui, &mut self.state);
        }
        if self.state.panels.right_panel {
            panels::right_panel::show(
                ui,
                &mut self.state,
                self.library.as_ref(),
                self.player.as_ref(),
            );
        }
        views::show(
            ui,
            &mut self.state,
            self.library.as_ref(),
            self.player.as_ref(),
        );

        self.apply_pending();
        self.tick_config_persistence();

        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if self.player.status() == PlaybackStatus::Playing && !minimized {
            ctx.request_repaint_after(PLAYING_REPAINT_INTERVAL);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let current = Config::capture(&self.state, self.player.as_ref());
        self.write_config(current);
    }
}
