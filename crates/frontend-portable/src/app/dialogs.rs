//! Owns the open tag-editor session and opens the app's message dialogs.

use std::cell::RefCell;
use std::rc::Rc;

use xui::xui_core::app::{Ui, WindowHandle};
use xui::xui_core::widget::{Dialog, DialogAction};

use super::Msg;
use super::Win32App;
use crate::dialogs::tag_editor;

/// An open tag editor window plus the bridge that carries its save request to
/// the main window and the shared model's status back.
pub(super) struct TagEditorSession {
    pub(super) bridge: Rc<RefCell<tag_editor::Bridge>>,
    pub(super) window: WindowHandle<tag_editor::Msg>,
}

impl Win32App {
    /// Opens the tag editor window for the editor state the shell just resolved
    /// (via [`Command::OpenTagEditor`](emusic_ui::state::Command::OpenTagEditor)),
    /// if none is open yet.
    pub(super) fn maybe_open_tag_editor(&mut self, ui: &Ui<Msg>) {
        if self.tag_editor.is_some() {
            return;
        }
        let Some(state) = self.shell.state.tag_editor.as_ref() else {
            return;
        };
        let bridge = Rc::new(RefCell::new(tag_editor::Bridge::new(state.status.clone())));
        match tag_editor::open(ui, state, Rc::clone(&bridge), ui.proxy()) {
            Ok(window) => {
                self.tag_editor = Some(TagEditorSession { bridge, window });
            }
            Err(error) => {
                tracing::warn!(%error, "could not open the tag editor window");
                self.shell.state.tag_editor = None;
            }
        }
    }

    /// Mirrors the shared model's editor status into the open bridge, so the
    /// dialog's poll sees a finished save.
    pub(super) fn mirror_tag_editor_status(&self) {
        if let Some(session) = &self.tag_editor
            && let Some(state) = self.shell.state.tag_editor.as_ref()
        {
            session.bridge.borrow_mut().status = state.status.clone();
        }
    }

    /// Shows the Clear-history confirmation dialog, if it is not already open.
    pub(super) fn show_history_clear(&mut self, ui: &Ui<Msg>) {
        if self
            .history_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.is_open())
        {
            return;
        }
        let dialog = Dialog::confirm(
            ui,
            "Clear play history?",
            "This removes every recorded play, including the most-played \
             rankings. Per-track play counts are kept. This cannot be undone.",
        )
        .expect("create clear-history dialog")
        .accept_label("Clear history")
        .on_action(|action| match action {
            DialogAction::Accept(_) => Some(Msg::HistoryClearConfirmed),
            DialogAction::Cancel => None,
        });
        dialog.open();
        self.history_dialog = Some(dialog);
    }

    /// Shows the Help -> Keyboard shortcuts message dialog, if it is not already
    /// open.
    pub(super) fn show_shortcuts(&mut self, ui: &Ui<Msg>) {
        if self
            .shortcuts_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.is_open())
        {
            return;
        }
        let dialog = Dialog::message(ui, "Keyboard shortcuts", &crate::dialogs::shortcuts_text())
            .expect("create keyboard shortcuts dialog")
            .on_action(|_| None);
        dialog.open();
        self.shortcuts_dialog = Some(dialog);
    }
}
