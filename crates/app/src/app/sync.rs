//! Pushes shell state into the window's views and applies model intents.

use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::player_api::PlayerApi;
use emusic_ui::shell::Changes;
use emusic_ui::state::{ShortcutAction, View, shortcut_command};
use emusic_ui::views::folders::FoldersMsg;
use emusic_ui::views::{Commands, Ctx};

use super::Win32App;
use crate::menu;
use crate::theme::app_theme;

impl Win32App {
    /// Pushes the current shell state into the views and reflects the theme.
    pub(super) fn sync_views(&mut self, changes: Changes) {
        let view = self.shell.state.view;
        let panels = self.shell.state.panels;
        let browser_visible = view == View::Music && self.shell.state.music.browser.visible;
        if view != self.applied_view
            || panels != self.applied_panels
            || browser_visible != self.applied_browser_visible
        {
            self.applied_view = view;
            self.applied_panels = panels;
            self.applied_browser_visible = browser_visible;
            self.apply_visibility();
            self.relayout();
        }

        let playing_id = playing_id(self.shell.library.as_ref(), self.shell.player.as_ref());
        match view {
            View::Music => {
                self.refresh_music();
                self.browser.sync(&self.shell.state.music.browser);
                self.music.sync(
                    &self.shell.state,
                    self.shell.library.as_ref(),
                    &self.shell.search,
                    playing_id,
                    changes,
                );
            }
            View::Folders => {
                self.refresh_folders();
                self.folders.sync(
                    &self.shell.state.folders,
                    self.shell.library.as_ref(),
                    playing_id,
                );
            }
            View::Albums => {
                if self.albums.sync(
                    &mut self.shell.state,
                    self.shell.library.as_ref(),
                    playing_id,
                    changes,
                ) {
                    self.relayout();
                }
            }
            View::Artists => self
                .artists
                .sync(&mut self.shell.state, self.shell.library.as_ref()),
            View::Genres => self
                .genres
                .sync(&mut self.shell.state, self.shell.library.as_ref()),
            View::Starred => {
                self.starred.sync(
                    &mut self.shell.state,
                    self.shell.library.as_ref(),
                    playing_id,
                );
            }
            View::MostPlayed => {
                self.most_played.sync(
                    &mut self.shell.state,
                    self.shell.library.as_ref(),
                    playing_id,
                );
            }
            View::History => {
                let rebuild = changes.intersects(Changes::LIBRARY);
                self.history
                    .sync(self.shell.library.as_ref(), playing_id, rebuild);
            }
            _ => {}
        }
        self.navigator.sync(view);
        menu::sync(&self.menu, &self.shell.state);
        if view == View::Settings {
            self.settings
                .sync(&self.shell.state, self.shell.library.as_ref());
        }

        // The transport and status models are shell state, synced from the
        // live player/library before the views read them.
        let player = self.shell.player.as_ref();
        self.shell.state.top_bar.sync(player);
        self.top_bar
            .sync(&self.shell.state.top_bar, &self.shell.state.search_query);
        self.shell.state.status_bar.sync(
            self.shell.state.search_result_count,
            self.shell.library.as_ref(),
            player,
        );
        let notice = self.shell.backend_notice();
        self.status_bar.sync(&self.shell.state.status_bar, notice);

        let look = (self.shell.state.theme, self.shell.state.accent);
        if look != self.applied_look {
            self.applied_look = look;
            self.ui.set_theme(app_theme(look.0, look.1));
        }
    }

    /// Shows the active central view and hides the others.
    pub(super) fn apply_visibility(&mut self) {
        let view = self.shell.state.view;
        let music = view == View::Music;
        let folders = view == View::Folders;
        let settings = view == View::Settings;
        self.music.set_visible(music);
        self.folders.set_visible(folders);
        self.albums.set_visible(view == View::Albums);
        self.artists.set_visible(view == View::Artists);
        self.genres.set_visible(view == View::Genres);
        self.starred.set_visible(view == View::Starred);
        self.most_played.set_visible(view == View::MostPlayed);
        self.history.set_visible(view == View::History);
        self.browser
            .set_visible(music && self.shell.state.music.browser.visible);
        self.settings.set_visible(settings);
        let ported = matches!(
            view,
            View::Music
                | View::Folders
                | View::Albums
                | View::Artists
                | View::Genres
                | View::Starred
                | View::MostPlayed
                | View::History
                | View::Settings
        );
        self.placeholder.set_visible(!ported);
        self.navigator
            .set_visible(self.shell.state.panels.navigator);
        self.status_bar
            .set_visible(self.shell.state.panels.status_bar);
        if !ported {
            self.placeholder.sync(&format!(
                "{} view: not ported to xui_core yet (see the migration epic #369)",
                view.label()
            ));
        }
    }

    /// Rebuilds the Folders model (its tree rows and visible ids) from the
    /// library snapshot.
    fn refresh_folders(&mut self) {
        let tracks: Vec<&TrackInfo> = self.shell.library.as_ref().tracks().iter().collect();
        let cx = Ctx::with_library(&tracks, None, self.shell.library.as_ref());
        self.shell.state.folders.refresh(&cx);
    }

    /// Rebuilds the Music model's column-browser facets, visible indices and
    /// search count from the library snapshot and the live search.
    fn refresh_music(&mut self) {
        let tracks: Vec<&TrackInfo> = self.shell.library.as_ref().tracks().iter().collect();
        self.shell.state.music.refresh(&tracks, &self.shell.search);
    }

    /// Applies a Folders intent through the shared model, dispatches the
    /// commands it emits, and refreshes the model.
    pub(super) fn apply_folders(&mut self, message: FoldersMsg) {
        let tracks: Vec<&TrackInfo> = self.shell.library.as_ref().tracks().iter().collect();
        let cx = Ctx::with_library(&tracks, None, self.shell.library.as_ref());
        let mut out = Commands::new();
        self.shell.state.folders.update(message, &cx, &mut out);
        for command in out.into_vec() {
            self.shell.dispatch(command);
        }
        self.refresh_folders();
    }

    /// Runs one keyboard-shortcut action through the shared table helper.
    pub(super) fn handle_shortcut(&mut self, action: ShortcutAction) {
        let player = self.shell.player.as_ref();
        let command = shortcut_command(
            action,
            player.position(),
            player.duration(),
            player.volume(),
        );
        if let Some(command) = command {
            self.shell.dispatch(command);
        }
    }
}

/// The library id of the player's current track, matched by path.
pub(super) fn playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library
        .track_by_path(&now_playing.path)
        .map(|track| track.id)
}
