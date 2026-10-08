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
use xui::xui_core::app::{Proxy, Ui};
use xui::xui_core::backend::WidgetId;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, CheckBox, Edit, HasText, Label, ListView};

use crate::app::Msg;

use super::SettingsMsg;
use super::form::{FormPage, HEADING_HEIGHT, LABEL_WIDTH, ROW_HEIGHT};
use crate::make::make;
use xui::xui_core::arrange::{button, checkbox, edit, label, list};

/// Height of the configured-server list, in design units.
const LIST_HEIGHT: f32 = 120.0;
/// Height of the explanatory hint, in design units.
const HINT_HEIGHT: f32 = 40.0;
/// Width of each action button, in design units.
const ACTION_WIDTH: f32 = 130.0;

/// The Server settings page's controls.
pub(super) struct ServerPage {
    form: FormPage,
    _heading: Label<Msg>,
    _hint: Label<Msg>,
    _url_label: Label<Msg>,
    url: Edit<Msg>,
    _name_label: Label<Msg>,
    name: Edit<Msg>,
    _code_label: Label<Msg>,
    code: Edit<Msg>,
    _pair: Button<Msg>,
    _sync: Button<Msg>,
    unpair: Button<Msg>,
    only: CheckBox<Msg>,
    list: ListView<Msg>,
    status: Label<Msg>,
    proxy: Proxy<Msg>,
    /// Servers the list was last built from, so it is only rebuilt on change.
    applied_servers: Vec<RemoteServer>,
    /// The page's own status line (pairing progress/outcome), shown when the
    /// backend has no status of its own.
    local_status: Option<String>,
}

impl ServerPage {
    /// Builds the page's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &Ui<Msg>) -> ServerPage {
        let form = FormPage::new(ui).expect("create server form");
        let proxy = ui.proxy();

        let heading = make(form.ui(), label("Homelab server")).expect("create server heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let hint = make(
            form.ui(),
            label("Pair with an emusic-server, then its library syncs in and plays on demand."),
        )
        .expect("create server hint");
        form.add_full(hint.id(), dip(HINT_HEIGHT));

        let url_row = form.row(ROW_HEIGHT).expect("create server url row");
        let url_label = make(url_row.ui(), label("Server URL")).expect("create url label");
        let url = make(url_row.ui(), edit()).expect("create url field");
        url_row
            .fixed(url_label.id(), LABEL_WIDTH)
            .fill(url.id())
            .finish();

        let name_row = form.row(ROW_HEIGHT).expect("create server name row");
        let name_label = make(name_row.ui(), label("Device name")).expect("create name label");
        let name = make(name_row.ui(), edit()).expect("create name field");
        name_row
            .fixed(name_label.id(), LABEL_WIDTH)
            .fill(name.id())
            .finish();

        let code_row = form.row(ROW_HEIGHT).expect("create server code row");
        let code_label = make(code_row.ui(), label("Pairing code")).expect("create code label");
        let code = make(code_row.ui(), edit()).expect("create code field");
        let pair = make(code_row.ui(), button("Pair"))
            .expect("create pair button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::ServerPair)));
        code_row
            .fixed(code_label.id(), LABEL_WIDTH)
            .fill(code.id())
            .fixed(pair.id(), dip(ACTION_WIDTH))
            .finish();

        let actions = form.row(ROW_HEIGHT).expect("create server action row");
        let sync = make(actions.ui(), button("Sync now"))
            .expect("create sync button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::ServerSync)));
        let unpair = make(actions.ui(), button("Unpair"))
            .expect("create unpair button")
            .on_click(|| Some(Msg::Settings(SettingsMsg::ServerUnpair)));
        actions
            .fixed(sync.id(), dip(ACTION_WIDTH))
            .fixed(unpair.id(), dip(ACTION_WIDTH))
            .finish();

        let only = make(form.ui(), checkbox("Server only (ignore local folders)"))
            .expect("create server-only checkbox")
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::ServerOnly(on))));
        form.add_full(only.id(), ROW_HEIGHT);

        let list = make(form.ui(), list()).expect("create server list");
        form.add_full(list.id(), dip(LIST_HEIGHT));

        let status = make(form.ui(), label("")).expect("create server status");
        form.add_full(status.id(), ROW_HEIGHT);

        ServerPage {
            form,
            _heading: heading,
            _hint: hint,
            _url_label: url_label,
            url,
            _name_label: name_label,
            name,
            _code_label: code_label,
            code,
            _pair: pair,
            _sync: sync,
            unpair,
            only,
            list,
            status,
            proxy,
            applied_servers: Vec::new(),
            local_status: None,
        }
    }

    /// The page's node identity (its scroll view).
    pub(super) fn id(&self) -> WidgetId {
        self.form.id()
    }

    /// Re-lays the page's form.
    pub(super) fn relayout(&self) {
        self.form.relayout();
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
            let items: Vec<String> = state
                .remote_servers
                .iter()
                .map(|server| format!("{} — {}", server.name, server.url))
                .collect();
            let rows: Vec<&str> = items.iter().map(String::as_str).collect();
            self.list.set_items(&rows);
            self.list.select(None);
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
    #[test]
    fn the_shared_pair_helper_reports_bad_urls() {
        // The page delegates to `emusic_ui::remote::pair`; a scheme-less URL
        // must fail without any network access.
        assert!(emusic_ui::remote::pair("pc", "music.example.com", "123456").is_err());
    }
}
