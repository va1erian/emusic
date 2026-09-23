//! The per-frame update loop ([`eframe::App`]) and config persistence for
//! [`App`].

use std::time::{Duration, Instant};

use eframe::egui;
use tracing::warn;

use crate::config::{self, Config};
use crate::player_api::PlaybackStatus;
use crate::{panels, theme, views};

use super::{App, commands};

/// Cap on repaint rate while playing, per #6 ("`request_repaint_after(33ms)`
/// only while playing and window not minimized").
const PLAYING_REPAINT_INTERVAL: Duration = Duration::from_millis(33);

/// How long after a settings change the config is written, per #8; further
/// changes within the window restart the countdown.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(2);

impl App {
    fn apply_pending(&mut self) {
        let commands = std::mem::take(&mut self.state.pending);
        for cmd in &commands {
            self.state.apply_local(cmd);
            commands::apply_player_command(self.player.as_mut(), self.library.as_ref(), cmd);
        }
        commands::apply_library_commands(self.library.as_mut(), &mut self.state, &commands);
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

        // Mirror the current track to the OS media overlay and fold any
        // transport events from it into this frame's queued commands (#26).
        if let Some(smtc) = self.smtc.as_mut() {
            smtc.sync(self.player.as_ref(), self.library.as_ref(), &mut self.state);
        }

        // Mirror the player's transport state onto the taskbar thumbnail
        // buttons and fold their presses into this frame's commands (#42).
        if let Some(thumbbar) = self.thumbbar.as_mut() {
            thumbbar.sync(self.player.as_ref(), &mut self.state);
        }

        theme::apply(&ctx, self.state.theme, self.state.accent.color());

        self.handle_shortcuts(&ctx);

        // Keep both search engines in sync: the top bar's (drives the Music
        // view's live filter) and the popup's (independent, so typing in
        // the popup never changes what's filtered underneath it). Both run
        // their matching on background threads (see `crate::search`), so
        // neither ever blocks a frame.
        self.search
            .tick(self.library.tracks(), &self.state.search_query);
        self.popup_search
            .tick(self.library.tracks(), &self.state.search_popup.query);

        self.poll_ipc(&ctx);

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
            panels::status_bar::show(
                ui,
                &mut self.state,
                self.library.as_ref(),
                self.player.as_ref(),
            );
        }
        if self.state.panels.navigator {
            panels::navigator::show(ui, &mut self.state, self.library.as_ref());
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
            &self.search,
        );
        panels::search_popup::show(
            &ctx,
            &mut self.state,
            self.library.as_ref(),
            &self.popup_search,
        );

        self.apply_pending();
        self.tick_config_persistence();

        // Repaint policy (#6, #25): ~30 fps while something is actually
        // playing and the window is visible (a minimized window needs no
        // frames). This is also what animates the status-bar visualizer; the
        // strip only reads FFT/samples while a mode is active *and* playing
        // (see `panels::visualizer`), so `Off` costs no audio reads even on
        // these repaint frames.
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
