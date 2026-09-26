//! Settings → Server page (#391): pair with an `emusic-server`, sync it and
//! unpair. Pairing runs on a background thread (the pairing request is over
//! the network) and reports back through [`SettingsMsg::ServerPaired`].
//!
//! The URL and pairing code are entered here; on success the server is queued
//! as [`Command::AddRemoteServer`], which the shell persists to the config and
//! hands to the library backend, which syncs it and downloads tracks on demand.

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::remote::{RemoteServer, pair};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Commands;
use win32ui::prelude::*;
use win32ui::{Button, CheckBox, Edit, Fill, ListView, Proxy, dip};

use crate::app::Msg;

use super::{FormRow, HEADING_HEIGHT, LABEL_WIDTH, ROW_HEIGHT, ScrollPanel, SettingsMsg};

/// Height of the configured-server list, in design units.
const LIST_HEIGHT: f32 = 120.0;
/// Width of each action button, in design units.
const ACTION_WIDTH: f32 = 120.0;
/// Width of the URL and name fields, in design units.
const FIELD_WIDTH: f32 = 320.0;
/// Height of a text field: one line of text (the default edit height is
/// taller than a row, which stretched the fields).
const FIELD_HEIGHT: f32 = 24.0;

/// The list model: one display line per configured server.
struct ServerModel {
    rows: Vec<String>,
}

impl ListModel for ServerModel {
    type Item = String;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> Option<&String> {
        self.rows.as_slice().get(index)
    }
}

/// The Server settings page's controls.
pub(super) struct ServerPage {
    form: ScrollPanel,
    heading: Label,
    hint: Label,
    url_label: Label,
    url: Edit<Msg>,
    name_label: Label,
    name: Edit<Msg>,
    code_label: Label,
    code: Edit<Msg>,
    pair: Button<Msg>,
    sync: Button<Msg>,
    unpair: Button<Msg>,
    only: CheckBox<Msg>,
    list: ListView<String, Msg>,
    status: Label,
    proxy: Proxy<Msg>,
    /// Servers the list was last built from, so it is only rebuilt on change.
    applied_servers: Vec<RemoteServer>,
    /// The page's own status line (pairing progress/outcome), shown when the
    /// backend has no status of its own.
    local_status: Option<String>,
}

impl ServerPage {
    /// Builds the page's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>, proxy: Proxy<Msg>) -> win32ui::Result<Self> {
        let form = ScrollPanel::new(ui)?;
        let mut panel = form.ui(ui);
        let heading = Label::new(&mut panel, Rect::default(), "Homelab server")?;
        let hint = Label::new(
            &mut panel,
            Rect::default(),
            "Pair with an emusic-server, then its library syncs in and plays on demand.",
        )?;
        let url_label = Label::new(&mut panel, Rect::default(), "Server URL")?;
        let url = Edit::single_line(&mut panel)?.cue("https://music.example.com");
        let name_label = Label::new(&mut panel, Rect::default(), "Device name")?;
        let name = Edit::single_line(&mut panel)?.cue("emusic on this PC");
        let code_label = Label::new(&mut panel, Rect::default(), "Pairing code")?;
        let code = Edit::single_line(&mut panel)?
            .cue("6 digits")
            .on_submit(|| Some(Msg::Settings(SettingsMsg::ServerPair)));
        let pair = Button::new(&mut panel, "Pair")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::ServerPair)));
        let sync = Button::new(&mut panel, "Sync now")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::ServerSync)));
        let unpair = Button::new(&mut panel, "Unpair")?
            .on_click(|| Some(Msg::Settings(SettingsMsg::ServerUnpair)));
        let only = CheckBox::new(&mut panel, "Server only (ignore local folders)")?
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::ServerOnly(on))));
        let list = ListView::new(&mut panel)?.column("Server", Fill, |row: &String| row.as_str());
        let status = Label::new(&mut panel, Rect::default(), "")?;

        let page = Self {
            form,
            heading,
            hint,
            url_label,
            url,
            name_label,
            name,
            code_label,
            code,
            pair,
            sync,
            unpair,
            only,
            list,
            status,
            proxy,
            applied_servers: Vec::new(),
            local_status: None,
        };
        page.apply(ui);
        Ok(page)
    }

    /// The page's scrollable form as one tab-strip page.
    pub(super) fn page(&self) -> LayoutItem {
        self.form.page()
    }

    /// The page's controls as form rows, in display order.
    fn rows(&self) -> Vec<FormRow> {
        let field = |label: &Label, control: LayoutItem| -> FormRow {
            (
                row![
                    label.width(dip(LABEL_WIDTH)),
                    Layout::row()
                        .spacing(dip(8.0))
                        .item(control)
                        .height(dip(FIELD_HEIGHT))
                ]
                .height(dip(FIELD_HEIGHT)),
                FIELD_HEIGHT,
            )
        };
        let url_row = field(&self.url_label, self.url.width(dip(FIELD_WIDTH)));
        let name_row = field(&self.name_label, self.name.width(dip(FIELD_WIDTH)));
        let code_row = field(
            &self.code_label,
            Layout::row()
                .spacing(dip(8.0))
                .item(self.code.width(dip(160.0)))
                .item(&self.pair)
                .into_layout_item(),
        );
        let mut actions = Layout::row().spacing(dip(8.0));
        for button in [&self.sync, &self.unpair] {
            actions = actions.item(button.width(dip(ACTION_WIDTH)));
        }
        vec![
            (self.heading.height(dip(HEADING_HEIGHT)), HEADING_HEIGHT),
            (self.hint.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            url_row,
            name_row,
            code_row,
            (actions.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (self.only.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
            (self.list.height(dip(LIST_HEIGHT)), LIST_HEIGHT),
            (self.status.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
        ]
    }

    /// Reinstalls the page's form (used after a visibility change).
    fn apply(&self, ui: &Ui<Msg>) {
        self.form.apply(ui, self.rows());
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.form.set_visible(visible);
    }

    /// Rebuilds the server list when the configured set changed, and mirrors
    /// the backend's sync status onto the page.
    pub(super) fn sync(&mut self, state: &AppState, library: &dyn LibraryDataSource) {
        if self.applied_servers != state.remote_servers {
            self.applied_servers = state.remote_servers.clone();
            let rows = state
                .remote_servers
                .iter()
                .map(|server| format!("{} — {}", server.name, server.url))
                .collect();
            self.list.set_model(ServerModel { rows });
            self.list.set_selection(&[]);
        }
        self.unpair.set_enabled(self.list.selected().is_some());
        self.only.set_checked(state.server_only);
        let backend_status = library.remote_status();
        let text = backend_status
            .or_else(|| self.local_status.clone())
            .unwrap_or_else(|| {
                if state.remote_servers.is_empty() {
                    "Not paired. Enter the server URL and a pairing code.".to_string()
                } else {
                    String::new()
                }
            });
        self.status.set_text(&text);
    }

    /// Handles the Server page's messages; returns whether `msg` was one.
    pub(super) fn update(&mut self, msg: &SettingsMsg, out: &mut Commands) -> bool {
        match msg {
            SettingsMsg::ServerPair => {
                let url = self.url.text();
                let name = self.name.text();
                let code = self.code.text();
                self.local_status = Some("Pairing...".to_string());
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = pair(&name, &url, &code);
                    let _ = proxy.send(Msg::Settings(SettingsMsg::ServerPaired(result)));
                });
            }
            SettingsMsg::ServerPaired(result) => match result {
                Ok((server, message)) => {
                    self.local_status = Some(message.clone());
                    out.push(Command::AddRemoteServer(server.clone()));
                }
                Err(error) => {
                    self.local_status = Some(format!("Pairing failed: {error}"));
                }
            },
            SettingsMsg::ServerSync => out.push(Command::SyncRemote),
            SettingsMsg::ServerOnly(on) => out.push(Command::SetServerOnly(*on)),
            SettingsMsg::ServerUnpair => {
                if let Some(id) = self
                    .list
                    .selected()
                    .and_then(|index| self.applied_servers.as_slice().get(index))
                    .map(|server| server.id.clone())
                {
                    out.push(Command::RemoveRemoteServer(id));
                }
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_pair_helper_reports_bad_urls() {
        // The page delegates to `emusic_ui::remote::pair`; a scheme-less URL
        // must fail without any network access.
        assert!(pair("pc", "music.example.com", "123456").is_err());
    }
}
