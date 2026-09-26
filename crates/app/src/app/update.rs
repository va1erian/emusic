//! The [`App`] implementation: message dispatch and post-update bookkeeping.

use emusic_ui::state::{Command, View};
use emusic_ui::views::folders::FoldersMsg;
use xui::xui_core::app::{App, Ui};

use super::Win32App;
use super::msg::{Msg, shell_command};
use crate::dialogs::{database_info, database_info::DatabaseInfoChoice, properties};
use crate::views::column_browser;
use crate::views::track_table::{self, ContextAction};

impl App for Win32App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        self.handle(msg, ui);
        self.after_update(ui);
    }
}

impl Win32App {
    /// Applies one message. Each arm runs the shell and syncs the views; the
    /// dialog bookkeeping runs afterwards in [`Win32App::after_update`].
    fn handle(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Wake | Msg::Timer => self.tick_inner(),
            Msg::Resize => self.relayout(),
            Msg::Dispatch(command) => {
                self.shell.dispatch(command);
                self.tick_inner();
            }
            Msg::Navigate(view) => {
                self.shell.dispatch(Command::SetView(view));
                self.tick_inner();
            }
            Msg::PlayRow(row) => {
                let command = match self.shell.state.view {
                    View::Music => self.music.activate(row),
                    View::Folders => self.folders.activate(row),
                    View::Starred => self.starred.activate(row),
                    View::MostPlayed => self.most_played.activate(row),
                    View::History => self.history.activate(row),
                    _ => None,
                };
                if let Some(command) = command {
                    self.shell.dispatch(command);
                    self.tick_inner();
                }
            }
            Msg::ToggleStarRow(row) => {
                let command = match self.shell.state.view {
                    View::Music => self.music.toggle_star(row),
                    View::Folders => self.folders.toggle_star(row),
                    View::Starred => self.starred.toggle_star(row),
                    View::MostPlayed => self.most_played.toggle_star(row),
                    _ => None,
                };
                if let Some(command) = command {
                    self.shell.dispatch(command);
                    self.tick_inner();
                }
            }
            Msg::SortColumn(column) => {
                let Some(id) = track_table::column_id(column) else {
                    return;
                };
                match self.shell.state.view {
                    View::Music => {
                        self.shell.state.music.table.sort.toggle(id);
                        self.music.resort(
                            &self.shell.state,
                            self.shell.library.as_ref(),
                            &self.shell.search,
                        );
                    }
                    View::Folders => {
                        self.shell.state.folders.table.sort.toggle(id);
                        self.folders
                            .resort(&self.shell.state.folders, self.shell.library.as_ref());
                    }
                    View::Starred => {
                        self.shell.state.starred.table.sort.toggle(id);
                        self.starred
                            .resort(&self.shell.state, self.shell.library.as_ref());
                    }
                    View::MostPlayed => {
                        self.shell.state.most_played.table.sort.toggle(id);
                        self.most_played
                            .resort(&self.shell.state, self.shell.library.as_ref());
                    }
                    _ => return,
                }
                self.tick_inner();
            }
            Msg::ContextRow(row) => {
                // The portable `ListView` reports the row but not the pointer,
                // so open near the central area's top-left corner.
                self.context_row = Some(row);
                let at = self.central_bounds;
                self.context_menu.show_context(at.left + 8, at.top + 8);
            }
            Msg::ContextAction(action) => {
                let Some(row) = self.context_row else {
                    return;
                };
                let track = match self.shell.state.view {
                    View::Music => self.music.track(row),
                    View::Folders => self.folders.track(row),
                    View::Starred => self.starred.track(row),
                    View::MostPlayed => self.most_played.track(row),
                    _ => None,
                };
                let Some(track) = track else {
                    return;
                };
                if action == ContextAction::Properties {
                    properties::show(ui, &track);
                    return;
                }
                let command = track_table::run_context_action(action, &track);
                if let Some(command) = command {
                    self.shell.dispatch(command);
                    self.tick_inner();
                }
            }
            Msg::DatabaseInfo => {
                let choice = database_info::show(ui, self.shell.library.as_ref());
                if choice == Some(DatabaseInfoChoice::Rescan) {
                    self.shell.dispatch(Command::LibraryRescan);
                    self.tick_inner();
                }
            }
            Msg::KeyboardShortcuts => self.show_shortcuts(ui),
            Msg::About => {
                self.shell.dispatch(Command::SetView(View::Settings));
                self.tick_inner();
            }
            Msg::SetAccent(accent) => {
                self.shell.dispatch(Command::SetAccent(accent));
                self.tick_inner();
            }
            Msg::TagEditorApply => {
                let Some(session) = &self.tag_editor else {
                    return;
                };
                let request = session.bridge.borrow_mut().apply.take();
                if let Some(request) = request {
                    self.shell.dispatch(Command::RequestTagEdits(vec![request]));
                    self.tick_inner();
                }
            }
            Msg::FoldersSelect(path) => {
                self.apply_folders(FoldersMsg::SelectNode(path));
                self.tick_inner();
            }
            Msg::FoldersSubfolders(include) => {
                self.apply_folders(FoldersMsg::SetIncludeSubfolders(include));
                self.tick_inner();
            }
            Msg::MusicShuffleAll => {
                let command = self.music.shuffle_all(
                    &self.shell.state,
                    self.shell.library.as_ref(),
                    &self.shell.search,
                );
                self.shell.dispatch(command);
                self.tick_inner();
            }
            Msg::BrowserRow { pane, rows } => {
                if column_browser::apply_selection(&mut self.shell.state.music.browser, pane, &rows)
                {
                    self.tick_inner();
                }
            }
            Msg::NameCountShuffle(row) => {
                let commands = match self.shell.state.view {
                    View::Artists => self.artists.shuffle(
                        row,
                        &mut self.shell.state,
                        self.shell.library.as_ref(),
                    ),
                    View::Genres => {
                        self.genres
                            .shuffle(row, &mut self.shell.state, self.shell.library.as_ref())
                    }
                    _ => Vec::new(),
                };
                for command in commands {
                    self.shell.dispatch(command);
                }
                self.tick_inner();
            }
            Msg::MostPlayed(window) => {
                self.shell.state.most_played.window = window;
                self.tick_inner();
            }
            Msg::HistoryClear => self.show_history_clear(ui),
            Msg::HistoryClearConfirmed => {
                self.shell.dispatch(Command::HistoryClear);
                self.tick_inner();
            }
            Msg::Shortcut(action) => {
                self.handle_shortcut(action);
                self.tick_inner();
            }
            Msg::Shell(action) => {
                if let Some(command) = shell_command(action) {
                    self.shell.dispatch(command);
                    self.tick_inner();
                }
            }
            Msg::Minimize => {
                if let Some(chrome) = &self.chrome {
                    chrome.minimize();
                }
            }
            Msg::ToggleMaximize => {
                if let Some(chrome) = &self.chrome {
                    chrome.toggle_maximize();
                }
            }
            Msg::Quit => {
                self.shell.save_on_exit();
                ui.close();
                // The app owns the close decision (`on_close` returns
                // `Some(Msg::Quit)`), so nothing else ends the event loop.
                ui.quit();
            }
        }
    }

    /// Runs the dialog bookkeeping after a message: reaps a closed tag editor,
    /// opens a pending one, mirrors its status, and drops a closed message
    /// dialog.
    fn after_update(&mut self, ui: &Ui<Msg>) {
        let closed = self
            .tag_editor
            .as_ref()
            .is_some_and(|session| !session.window.is_open());
        if closed {
            self.tag_editor = None;
            self.shell.state.tag_editor = None;
        }
        self.maybe_open_tag_editor(ui);
        self.mirror_tag_editor_status();
        if self
            .shortcuts_dialog
            .as_ref()
            .is_some_and(|dialog| !dialog.is_open())
        {
            self.shortcuts_dialog = None;
        }
        if self
            .history_dialog
            .as_ref()
            .is_some_and(|dialog| !dialog.is_open())
        {
            self.history_dialog = None;
        }
    }
}
