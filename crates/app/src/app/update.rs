//! The per-frame `eframe::App` loop for [`EguiApp`]: draw the shell's state,
//! then map its `next_wake` onto egui's repaint scheduling.

use std::time::Duration;

use eframe::egui;

use crate::state::Command;
use crate::{panels, views};

use super::EguiApp;

impl eframe::App for EguiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Driven by egui's own (deterministic, harness-controllable) frame
        // delta rather than a wall-clock `Instant`, so headless renders
        // (#32) are reproducible instead of depending on real elapsed time.
        let dt = ctx.input(|i| i.stable_dt).max(0.0);
        self.synthetic_now += Duration::from_secs_f32(dt);

        // Mirror the current track to the OS media overlay and fold any
        // transport events from it into this frame's queued commands (#26).
        if let Some(smtc) = self.smtc.as_mut() {
            smtc.sync(
                self.shell.player.as_ref(),
                self.shell.library.as_ref(),
                &mut self.shell.state,
            );
        }

        // Mirror the player's transport state onto the taskbar thumbnail
        // buttons and fold their presses into this frame's commands (#42).
        if let Some(thumbbar) = self.thumbbar.as_mut() {
            thumbbar.sync(self.shell.player.as_ref(), &mut self.shell.state);
        }

        self.handle_shortcuts(&ctx);
        // Remember the window size/position for the next launch (#214).
        self.record_window_geometry(&ctx);

        // Poll backends, apply the frame's queued commands and persist config.
        let tick = self.shell.tick(self.synthetic_now);
        if self.shell.take_focus_request() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        self.sync_appearance(&ctx);

        self.menu_bar(ui);
        if let Some(notice) = self.shell.backend_notice() {
            egui::Panel::top("backend_notice")
                .exact_size(24.0)
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.colored_label(ui.visuals().warn_fg_color, notice);
                    });
                });
        }
        panels::top_bar::show(ui, &mut self.shell.state, self.shell.player.as_ref());
        if self.shell.state.panels.status_bar {
            panels::status_bar::show(
                ui,
                &mut self.shell.state,
                self.shell.library.as_ref(),
                self.shell.player.as_ref(),
            );
        }
        if self.shell.state.panels.navigator {
            panels::navigator::show(ui, &mut self.shell.state, self.shell.library.as_ref());
        }
        if self.shell.state.panels.right_panel {
            panels::right_panel::show(
                ui,
                &mut self.shell.state,
                &mut self.images,
                self.shell.library.as_ref(),
                self.shell.player.as_ref(),
            );
        }
        // The Folders view's directory tree is a real top-level panel (like
        // the navigator/right panel above), not nested inside the central
        // view's ScrollArea — see `views::folders::tree_panel`.
        if self.shell.state.view == crate::state::View::Folders {
            let commands = views::folders::tree_panel(
                ui,
                &mut self.shell.state.folders,
                self.shell.library.as_ref(),
            );
            self.shell.state.pending.extend(commands.into_vec());
        }
        views::show(
            ui,
            &mut self.shell.state,
            &mut self.images,
            self.shell.library.as_ref(),
            self.shell.player.as_ref(),
            &self.shell.search,
        );
        // The now-playing "Properties" link can be clicked from any view, so
        // its dialog is rendered once here rather than by a single view.
        crate::views::track_table::show_properties(
            &ctx,
            &mut self.shell.state.now_playing.properties,
        );
        panels::database_info::show(&ctx, &mut self.shell.state, self.shell.library.as_ref());
        panels::search_popup::show(
            &ctx,
            &mut self.shell.state,
            self.shell.library.as_ref(),
            &self.shell.popup_search,
        );

        // The single-track tag editor (#172); a valid Apply becomes a queued
        // tag edit and Auto-tag a queued lookup (#209), both applied to the
        // library with the next tick's commands.
        match crate::tag_editor::show(&ctx, &mut self.shell.state.tag_editor) {
            Some(crate::tag_editor::TagEditorAction::Apply(request)) => {
                self.shell.dispatch(Command::RequestTagEdits(vec![request]));
            }
            Some(crate::tag_editor::TagEditorAction::AutoTag(request)) => {
                self.shell.dispatch(Command::AutoTagTrack {
                    path: request.path,
                    query: request.query,
                });
            }
            None => {}
        }

        // Repaint policy (#6, #25): the shell reports when it next needs a
        // frame (~1 s while playing, `FRAME_INTERVAL` while the visualizer
        // animates, `None` when idle). A minimized window needs no frames.
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if !minimized && let Some(wait) = tick.next_wake {
            ctx.request_repaint_after(wait);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shell.save_on_exit();
    }
}

impl EguiApp {
    /// Records the live window size/position into the shared state, so the
    /// config written on exit restores it next launch (#214). A maximized
    /// window records only the flag, keeping the last normal geometry.
    fn record_window_geometry(&mut self, ctx: &egui::Context) {
        let (inner, outer, maximized) = ctx.input(|i| {
            let viewport = i.viewport();
            (
                viewport.inner_rect,
                viewport.outer_rect,
                viewport.maximized.unwrap_or(false),
            )
        });
        let window = &mut self.shell.state.window;
        window.maximized = maximized;
        if maximized {
            return;
        }
        if let Some(rect) = inner {
            window.size = Some([rect.width(), rect.height()]);
        }
        if let Some(rect) = outer {
            window.position = Some([rect.min.x, rect.min.y]);
        }
    }
}
