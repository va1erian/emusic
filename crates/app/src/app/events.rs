//! egui input handling for [`EguiApp`]: the global search-popup shortcuts
//! and the menu bar. IPC polling/message handling lives in the shell (#97).

use eframe::egui;

use crate::state::{Command, PanelKind, View};

use super::EguiApp;

impl EguiApp {
    /// Handles the global search-popup shortcuts for this frame, before any
    /// panels are drawn.
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Checked (and consumed) before the top bar's own Ctrl+F check, and
        // matched most-specific-first, so Ctrl+Shift+F never also triggers
        // the plain Ctrl+F focus request.
        let toggle_popup = ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::F,
            ) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)
        });
        if toggle_popup {
            if self.shell.state.search_popup.state.open {
                self.shell.state.search_popup.state.close();
            } else {
                self.shell.state.search_popup.state.open();
            }
        }
    }

    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::containers::menu::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Database info...").clicked() {
                        self.shell.state.database_info_open = true;
                        ui.close();
                    }
                    if ui.button("Settings").clicked() {
                        self.shell.state.push(Command::SetView(View::Settings));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.send_viewport_cmd(egui::ViewportCommand::Close);
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    self.panel_menu_item(ui, "Navigator", PanelKind::Navigator);
                    self.panel_menu_item(ui, "Now playing panel", PanelKind::RightPanel);
                    self.panel_menu_item(ui, "Status bar", PanelKind::StatusBar);
                    ui.separator();
                    let mut column_browser = self.shell.state.music.browser.visible;
                    if ui.checkbox(&mut column_browser, "Column browser").changed() {
                        self.shell.state.push(Command::ToggleColumnBrowser);
                    }
                    ui.separator();
                    if ui.button("Toggle dark / light theme").clicked() {
                        self.shell.state.push(Command::ToggleTheme);
                        ui.close();
                    }
                });
            });
        });
    }

    fn panel_menu_item(&mut self, ui: &mut egui::Ui, label: &str, kind: PanelKind) {
        let visible = match kind {
            PanelKind::Navigator => self.shell.state.panels.navigator,
            PanelKind::RightPanel => self.shell.state.panels.right_panel,
            PanelKind::StatusBar => self.shell.state.panels.status_bar,
        };
        let mut checked = visible;
        if ui.checkbox(&mut checked, label).changed() {
            self.shell.state.push(Command::TogglePanel(kind));
        }
    }
}
