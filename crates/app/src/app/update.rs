//! The per-frame update loop ([`eframe::App`]) and config persistence for
//! [`App`].

use std::time::{Duration, Instant};

use eframe::egui;
use tracing::warn;

use crate::config::{self, Config};
use crate::player_api::PlaybackStatus;
use crate::state::VisualizerMode;
use crate::{panels, theme, views};

use super::{App, commands};

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
        // Route finished tag edits to the editor that requested them (#172),
        // so it can clear its pending state or show the per-file error.
        let tag_edit_results = self.library.take_tag_edit_results();
        crate::tag_editor::deliver(&mut self.state.tag_editor, tag_edit_results);

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

        theme::apply(&ctx, self.state.theme, self.state.accent);

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
        // The Folders view's directory tree is a real top-level panel (like
        // the navigator/right panel above), not nested inside the central
        // view's ScrollArea — see `views::folders::tree_panel`.
        if self.state.view == crate::state::View::Folders {
            views::folders::tree_panel(ui, &mut self.state, self.library.as_ref());
        }
        views::show(
            ui,
            &mut self.state,
            self.library.as_ref(),
            self.player.as_ref(),
            &self.search,
        );
        // The now-playing "Properties" link can be clicked from any view, so
        // its dialog is rendered once here rather than by a single view.
        crate::views::track_table::show_properties(&ctx, &mut self.state.now_playing.properties);
        panels::database_info::show(&ctx, &mut self.state, self.library.as_ref());
        panels::search_popup::show(
            &ctx,
            &mut self.state,
            self.library.as_ref(),
            &self.popup_search,
        );

        // The single-track tag editor (#172); a valid Apply becomes a queued
        // request, applied to the library with the rest of the frame's
        // commands below.
        if let Some(request) = crate::tag_editor::show(&ctx, &mut self.state.tag_editor) {
            self.state
                .push(crate::state::Command::RequestTagEdits(vec![request]));
        }

        self.apply_pending();
        self.tick_config_persistence();

        // Repaint policy (#6, #25): while something is playing and the window
        // is visible (a minimized window needs no frames), keep the elapsed
        // time / progress readouts roughly in step with a coarse one-second
        // tick. Only when the optional visualizer strip is both enabled and
        // animating (mode != Off) do we animate faster, at
        // `panels::visualizer::FRAME_INTERVAL` — a fixed, low rate rather than
        // the compositor's own refresh rate. Chasing vsync repainted the
        // *entire* window (immediate-mode redraws everything, not just the
        // 18 px strip) as fast as the monitor allowed, which on a 120/144 Hz
        // display burned CPU for no visible benefit and could miss frames
        // under load, showing up as flicker. This keeps an idle-looking
        // player at ~0% CPU even though it is "playing".
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if self.player.status() == PlaybackStatus::Playing && !minimized {
            if self.state.visualizer_enabled && self.state.visualizer != VisualizerMode::Off {
                ctx.request_repaint_after(panels::visualizer::FRAME_INTERVAL);
            } else {
                ctx.request_repaint_after(Duration::from_secs(1));
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let mut current = Config::capture(&self.state, self.player.as_ref());
        // The session (#190) is only snapshotted here, not every frame, so
        // the advancing playback position never dirties the settings.
        if self.state.resume_playback {
            current.last_played = crate::config::LastPlayed::capture(self.player.as_ref());
        }
        self.write_config(current);
    }
}
