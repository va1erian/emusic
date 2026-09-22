//! The [`eframe::App`] implementation: wires the menu bar, the four fixed
//! panels and the central view router together, applies queued
//! [`Command`]s, and implements the repaint policy from #6 (no continuous
//! repaint; ~30 fps only while something is actually playing).

use std::time::Duration;

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::player_api::{PlaybackStatus, PlayerApi, RepeatMode};
use crate::state::{AppState, Command, PanelKind, View};
use crate::{fonts, panels, theme, views};

/// Cap on repaint rate while playing, per #6 ("`request_repaint_after(33ms)`
/// only while playing and window not minimized").
const PLAYING_REPAINT_INTERVAL: Duration = Duration::from_millis(33);

pub struct App {
    state: AppState,
    library: Box<dyn LibraryDataSource>,
    player: Box<dyn PlayerApi>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        library: Box<dyn LibraryDataSource>,
        player: Box<dyn PlayerApi>,
    ) -> Self {
        fonts::install(&cc.egui_ctx);
        let state = AppState::default();
        theme::apply(&cc.egui_ctx, state.theme);
        Self {
            state,
            library,
            player,
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
            apply_player_command(self.player.as_mut(), cmd);
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

        // Driven by egui's own (deterministic, harness-controllable) frame
        // delta rather than a wall-clock `Instant`, so headless renders
        // (#32) are reproducible instead of depending on real elapsed time.
        let dt = ctx.input(|i| i.stable_dt);
        self.player.tick(Duration::from_secs_f32(dt.max(0.0)));

        theme::apply(&ctx, self.state.theme);

        self.menu_bar(ui);
        panels::top_bar::show(ui, &mut self.state, self.player.as_ref());
        if self.state.panels.status_bar {
            panels::status_bar::show(ui, self.library.as_ref(), self.player.as_ref());
        }
        if self.state.panels.navigator {
            panels::navigator::show(ui, &mut self.state);
        }
        if self.state.panels.right_panel {
            panels::right_panel::show(ui, self.player.as_ref());
        }
        views::show(
            ui,
            &mut self.state,
            self.library.as_ref(),
            self.player.as_ref(),
        );

        self.apply_pending();

        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if self.player.status() == PlaybackStatus::Playing && !minimized {
            ctx.request_repaint_after(PLAYING_REPAINT_INTERVAL);
        }
    }
}

fn apply_player_command(player: &mut dyn PlayerApi, cmd: &Command) {
    match cmd {
        Command::PlayerPlayPause => player.play_pause(),
        Command::PlayerStop => player.stop(),
        Command::PlayerNext => player.next(),
        Command::PlayerPrevious => player.previous(),
        Command::PlayerSeek(pos) => player.seek(*pos),
        Command::PlayerSetVolume(v) => player.set_volume(*v),
        Command::PlayerToggleRepeat => player.set_repeat_mode(next_repeat(player.repeat_mode())),
        Command::PlayerToggleShuffle => player.set_shuffle(!player.shuffle()),
        _ => {}
    }
}

fn next_repeat(mode: RepeatMode) -> RepeatMode {
    mode.next()
}
