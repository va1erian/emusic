//! Input and external-event handling for [`App`]: the global search-popup
//! shortcuts, messages forwarded by secondary instances (#11), and the menu
//! bar.

use eframe::egui;

use crate::backend::ipc::IpcBridge;
use crate::state::{Command, PanelKind, View};

use super::App;

impl App {
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
            if self.state.search_popup.open {
                self.state.search_popup.close();
            } else {
                self.state.search_popup.open();
            }
        }
    }

    /// Polls messages forwarded by a secondary instance (#11) and applies
    /// them, bringing the window to the foreground if there were any.
    pub(super) fn poll_ipc(&mut self, ctx: &egui::Context) {
        if let Some(message) = self.ipc.as_ref().and_then(IpcBridge::try_recv) {
            self.handle_ipc_message(message);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
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

    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::containers::menu::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Database info...").clicked() {
                        self.state.database_info_open = true;
                        ui.close();
                    }
                    if ui.button("Settings").clicked() {
                        self.state.push(Command::SetView(View::Settings));
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
                    let mut column_browser = self.state.column_browser.visible;
                    if ui.checkbox(&mut column_browser, "Column browser").changed() {
                        self.state.push(Command::ToggleColumnBrowser);
                    }
                    ui.separator();
                    if ui.button("Toggle dark / light theme").clicked() {
                        self.state.push(Command::ToggleTheme);
                        ui.close();
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
