//! The main window's menu bar and the track row context menu (#106, #262,
//! #376), on the portable [`Menu`].
//!
//! The bar's top-level entries are File/View/Help; the View toggles are
//! checkable, so the bar shows a tick for the panels, the column browser and
//! the theme. Commands map to the app's [`Msg`] like every other widget event.
//!
//! The track context menu is built once (the widget pools its popups) and shown
//! at a client point with [`Menu::show_context`] when a row is right-clicked.
//! The portable `ListView` reports the row and the pointer, so the menu opens
//! at the pointer, translated from the list's node-local point to the window.

use emusic_ui::state::{AppState, Command, PanelKind, View};
use xui::xui_core::app::Ui;
use xui::xui_core::widget::{Menu, MenuId};

use crate::app::Msg;
use crate::make::make;
use crate::views::track_table::ContextAction;
use xui::xui_core::arrange::menu_bar;

/// The height reserved for the menu bar, in device-independent pixels.
pub const MENU_BAR_HEIGHT: f32 = 26.0;

/// The opaque ids the bar and the context menu map back to commands.
mod id {
    use xui::xui_core::widget::MenuId;

    pub(super) const FILE_DATABASE_INFO: MenuId = MenuId::new(1);
    pub(super) const FILE_SETTINGS: MenuId = MenuId::new(2);
    pub(super) const FILE_QUIT: MenuId = MenuId::new(3);

    pub(super) const VIEW_NAVIGATOR: MenuId = MenuId::new(10);
    pub(super) const VIEW_RIGHT_PANEL: MenuId = MenuId::new(11);
    pub(super) const VIEW_NEXT_TRACKS: MenuId = MenuId::new(12);
    pub(super) const VIEW_STATUS_BAR: MenuId = MenuId::new(13);
    pub(super) const VIEW_COLUMN_BROWSER: MenuId = MenuId::new(14);
    pub(super) const VIEW_THEME: MenuId = MenuId::new(15);

    pub(super) const HELP_SHORTCUTS: MenuId = MenuId::new(20);
    pub(super) const HELP_ABOUT: MenuId = MenuId::new(21);

    pub(super) const CTX_PLAY: MenuId = MenuId::new(30);
    pub(super) const CTX_PLAY_NEXT: MenuId = MenuId::new(31);
    pub(super) const CTX_ADD_QUEUE: MenuId = MenuId::new(32);
    pub(super) const CTX_TOGGLE_STAR: MenuId = MenuId::new(33);
    pub(super) const CTX_COPY_PATH: MenuId = MenuId::new(34);
    pub(super) const CTX_OPEN_LOCATION: MenuId = MenuId::new(35);
    pub(super) const CTX_PROPERTIES: MenuId = MenuId::new(36);
    pub(super) const CTX_EDIT_TAGS: MenuId = MenuId::new(37);

    pub(super) const QUEUE_REMOVE: MenuId = MenuId::new(40);
}

/// Builds the File/View/Help menu bar, ticking the toggles that are
/// currently on; the app places it.
#[must_use]
pub fn bar(ui: &Ui<Msg>, state: &AppState) -> Menu<Msg> {
    let panels = state.panels;
    let browser_visible = state.music.browser.visible;
    let builder = menu_bar(move |m| {
        m.submenu(MenuId::new(0), "&File", |f| {
            f.item(id::FILE_DATABASE_INFO, "&Database info...");
            f.item(id::FILE_SETTINGS, "&Settings");
            f.separator();
            f.item(id::FILE_QUIT, "&Quit");
        });
        m.submenu(MenuId::new(4), "&View", |v| {
            v.check(id::VIEW_NAVIGATOR, "&Navigator", panels.navigator);
            v.check(
                id::VIEW_RIGHT_PANEL,
                "Now &playing panel",
                panels.right_panel,
            );
            v.check(id::VIEW_NEXT_TRACKS, "&Next tracks", panels.next_tracks);
            v.check(id::VIEW_STATUS_BAR, "&Status bar", panels.status_bar);
            v.separator();
            v.check(id::VIEW_COLUMN_BROWSER, "&Column browser", browser_visible);
            v.separator();
            v.item(id::VIEW_THEME, "Toggle &dark / light theme");
        });
        m.submenu(MenuId::new(5), "&Help", |h| {
            h.item(id::HELP_SHORTCUTS, "&Keyboard shortcuts...");
            h.item(id::HELP_ABOUT, "&About emusic");
        });
    });
    let menu = make(ui, builder)
        .expect("create the menu bar")
        .on_select(bar_select)
        .on_toggle(bar_toggle);
    sync(&menu, state);
    menu
}

/// The menu bar's plain commands.
fn bar_select(id: MenuId) -> Option<Msg> {
    match id {
        id::FILE_DATABASE_INFO => Some(Msg::DatabaseInfo),
        id::FILE_SETTINGS => Some(Msg::Dispatch(Command::SetView(View::Settings))),
        id::FILE_QUIT => Some(Msg::Quit),
        id::VIEW_THEME => Some(Msg::Dispatch(Command::ToggleTheme)),
        id::HELP_SHORTCUTS => Some(Msg::KeyboardShortcuts),
        id::HELP_ABOUT => Some(Msg::About),
        _ => None,
    }
}

/// The menu bar's check toggles; the command flips the state, so the new
/// checked value is ignored (the next [`sync`] restores the real one).
fn bar_toggle(id: MenuId, _checked: bool) -> Option<Msg> {
    let panel = match id {
        id::VIEW_NAVIGATOR => PanelKind::Navigator,
        id::VIEW_RIGHT_PANEL => PanelKind::RightPanel,
        id::VIEW_NEXT_TRACKS => PanelKind::NextTracks,
        id::VIEW_STATUS_BAR => PanelKind::StatusBar,
        id::VIEW_COLUMN_BROWSER => {
            return Some(Msg::Dispatch(Command::ToggleColumnBrowser));
        }
        _ => return None,
    };
    Some(Msg::Dispatch(Command::TogglePanel(panel)))
}

/// Re-installs the View ticks from the shell state, in place, so the bar is
/// not rebuilt when a panel toggles.
pub fn sync(menu: &Menu<Msg>, state: &AppState) {
    menu.set_checked(id::VIEW_NAVIGATOR, state.panels.navigator);
    menu.set_checked(id::VIEW_RIGHT_PANEL, state.panels.right_panel);
    menu.set_checked(id::VIEW_NEXT_TRACKS, state.panels.next_tracks);
    menu.set_checked(id::VIEW_STATUS_BAR, state.panels.status_bar);
    menu.set_checked(id::VIEW_COLUMN_BROWSER, state.music.browser.visible);
}

/// The track table's right-click menu.
#[must_use]
pub fn track_context(ui: &Ui<Msg>) -> Menu<Msg> {
    Menu::context(ui).on_select(context_select).build(|m| {
        m.item(id::CTX_PLAY, "&Play");
        m.item(id::CTX_PLAY_NEXT, "Play &next");
        m.item(id::CTX_ADD_QUEUE, "&Add to queue");
        m.separator();
        m.item(id::CTX_TOGGLE_STAR, "Star / &unstar");
        m.separator();
        m.item(id::CTX_COPY_PATH, "&Copy path");
        m.item(id::CTX_OPEN_LOCATION, "Open file &location");
        m.separator();
        m.item(id::CTX_PROPERTIES, "P&roperties");
        m.item(id::CTX_EDIT_TAGS, "&Edit tags...");
    })
}

/// The now-playing queue's right-click menu (#247, #451): remove the entry.
/// `remove` builds the surface's own remove message, so the central view and
/// the right panel can tell their two lists apart.
#[must_use]
pub fn queue_context(ui: &Ui<Msg>, remove: fn() -> Msg) -> Menu<Msg> {
    Menu::context(ui)
        .on_select(move |menu_id| match menu_id {
            id::QUEUE_REMOVE => Some(remove()),
            _ => None,
        })
        .build(|m| {
            m.item(id::QUEUE_REMOVE, "&Remove from queue");
        })
}

/// Maps a context entry to its [`ContextAction`].
fn context_select(id: MenuId) -> Option<Msg> {
    let action = match id {
        id::CTX_PLAY => ContextAction::Play,
        id::CTX_PLAY_NEXT => ContextAction::PlayNext,
        id::CTX_ADD_QUEUE => ContextAction::AddToQueue,
        id::CTX_TOGGLE_STAR => ContextAction::ToggleStar,
        id::CTX_COPY_PATH => ContextAction::CopyPath,
        id::CTX_OPEN_LOCATION => ContextAction::OpenFileLocation,
        id::CTX_PROPERTIES => ContextAction::Properties,
        id::CTX_EDIT_TAGS => ContextAction::EditTags,
        _ => return None,
    };
    Some(Msg::ContextAction(action))
}

/// Menu tests (#376): the bar's ticks and the context popup, on a real window.
/// Each test quits as soon as it has asserted, so no window is left open.
#[cfg(all(test, windows))]
mod tests {
    use std::rc::Rc;

    use emusic_ui::state::AppState;
    use xui::xui_core::app::App;
    use xui::xui_core::backend::{Backend, PlatformSpec};

    use super::*;

    /// An app that only hosts the widgets the test builds.
    struct TestApp;

    impl App for TestApp {
        type Msg = Msg;

        fn update(&mut self, _msg: Msg, _ui: &mut Ui<Msg>) {}
    }

    /// Runs `test` against a live window and quits.
    fn with_ui(test: impl FnOnce(&Ui<Msg>) + 'static) {
        let backend: Rc<dyn Backend> = Rc::new(xui::xui_win32::Win32Backend::new());
        let result = xui::xui_core::run_app(backend, PlatformSpec::new("menu test"), move |ui| {
            test(ui);
            ui.quit();
            TestApp
        });
        if result.is_err() {
            eprintln!("skipping: this session cannot create windows");
        }
    }

    #[test]
    fn bar_mirrors_the_panel_state() {
        with_ui(|ui| {
            let mut state = AppState::default();
            state.panels.navigator = false;
            let menu = bar(ui, &state);
            assert!(!menu.is_checked(id::VIEW_NAVIGATOR));
            assert_eq!(
                menu.is_checked(id::VIEW_STATUS_BAR),
                state.panels.status_bar
            );

            state.panels.navigator = true;
            state.music.browser.visible = !state.music.browser.visible;
            sync(&menu, &state);
            assert!(menu.is_checked(id::VIEW_NAVIGATOR));
            assert_eq!(
                menu.is_checked(id::VIEW_COLUMN_BROWSER),
                state.music.browser.visible
            );
        });
    }

    #[test]
    fn context_menu_opens_and_closes() {
        with_ui(|ui| {
            let menu = track_context(ui);
            assert!(!menu.is_open());
            menu.show_context(8, 8);
            assert!(menu.is_open(), "the context popup opened");
            menu.close();
            assert!(!menu.is_open(), "the context popup closed");
        });
    }
}
