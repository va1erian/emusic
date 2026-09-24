//! Toolkit-agnostic application controller (#97).
//!
//! [`Shell`] owns the shared [`AppState`], the library/player backends and
//! the two search engines, drains IPC, applies queued [`Command`]s and
//! persists the config (debounced, plus on exit). Frontends drive it once per
//! frame with [`Shell::tick`], draw [`Shell::state`] themselves, and map
//! [`Tick::next_wake`] onto their own repaint scheduling. [`Tick::changes`]
//! reports what changed since the previous tick so a retained-mode frontend
//! can update only the affected controls; an immediate-mode one ignores it.

mod commands;
mod tick;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tracing::warn;

use crate::backend::ipc::{self, IpcBridge};
use crate::config::{self, Config, LastPlayed};
use crate::library_api::LibraryDataSource;
use crate::panels::visualizer::FRAME_INTERVAL;
use crate::player_api::{PlaybackStatus, PlayerApi};
use crate::search::SearchEngine;
use crate::state::{Accent, AppState, Command, PanelVisibility, Theme, View, VisualizerMode};
use crate::tag_editor::{self, Status as TagStatus};
use crate::waker::WakerSlot;

pub use tick::{Changes, Tick};

/// How long after a settings change the config is written, per #8; further
/// changes within the window restart the countdown.
pub const SAVE_DEBOUNCE: Duration = Duration::from_secs(2);

/// How often the shell asks to be woken while a tag edit is in flight, so the
/// backend's outcome is picked up even when nothing else is moving.
pub const TAG_EDIT_REPAINT: Duration = Duration::from_millis(100);

/// The application controller shared by every frontend.
pub struct Shell {
    /// Everything the frontend's views read and queue commands into.
    pub state: AppState,
    pub library: Box<dyn LibraryDataSource>,
    pub player: Box<dyn PlayerApi>,
    /// Live library search backing the top bar / Music view.
    pub search: SearchEngine,
    /// Independent search backing the global search popup.
    pub popup_search: SearchEngine,
    /// Where the config is persisted; `None` disables all disk I/O (used by
    /// `emusic-shot` and the snapshot tests for determinism).
    config_path: Option<PathBuf>,
    /// Config as last loaded/saved, compared each tick to detect changes.
    saved: Config,
    /// When the live settings first diverged from `saved`; drives the
    /// debounced save.
    dirty_since: Option<Instant>,
    /// Set when this process is the primary instance (#11); polled once per
    /// tick for messages a secondary launch forwarded.
    ipc: Option<IpcBridge>,
    /// A startup problem to keep showing the user (e.g. "no audio device")
    /// rather than silently degrading; `None` once nothing is wrong.
    backend_notice: Option<String>,
    /// When [`Shell::tick`] last ran, so it can derive `dt` from `now`.
    last_tick: Option<Instant>,
    /// Observed display state at the previous tick, for [`Changes`].
    observed: Observed,
    /// Set when IPC asked the window to come forward; the frontend consumes
    /// it with [`Shell::take_focus_request`].
    focus_requested: bool,
}

/// Compact copy of the display-relevant state, diffed each tick to build
/// [`Changes`]. Deliberately coarse: a retained-mode frontend only needs to
/// know which *area* to re-read, not an exact field-by-field diff.
#[derive(Default, PartialEq)]
struct Observed {
    track_count: usize,
    scanning: bool,
    status_text: Option<String>,
    history_len: usize,
    now_playing_path: Option<String>,
    playback: PlaybackStatus,
    position_ms: u64,
    queue_len: usize,
    queue_head: Option<String>,
    search_active: bool,
    search_count: Option<usize>,
    popup_search_active: bool,
    popup_search_count: Option<usize>,
    view: View,
    theme: Theme,
    accent: Accent,
    panels: PanelVisibility,
}

impl Shell {
    /// Builds the shell, applying `config` to a fresh [`AppState`] and the
    /// player, and pointing the library at the configured folders. `waker` is
    /// handed to the search workers so a finished query wakes the frontend.
    ///
    /// A normal launch loads and persists `config_path`; passing `None`
    /// disables persistence (used by `emusic-shot` and the snapshot tests).
    pub fn new(
        mut library: Box<dyn LibraryDataSource>,
        mut player: Box<dyn PlayerApi>,
        mut config: Config,
        config_path: Option<PathBuf>,
        waker: WakerSlot,
    ) -> Self {
        let mut state = AppState::default();
        config.apply_to_state(&mut state);
        config.apply_to_player(player.as_mut());
        library.set_folders(&config.library_folders);
        // The session was just applied to the player; drop it from the
        // baseline so the per-tick settings compare doesn't treat the
        // (now-consumed) session as a pending change. It is written again on
        // exit (#190).
        config.last_played = None;

        let handle = waker.handle();
        Self {
            state,
            library,
            player,
            search: SearchEngine::with_waker(handle.clone()),
            popup_search: SearchEngine::with_waker(handle),
            config_path,
            saved: config,
            dirty_since: None,
            ipc: None,
            backend_notice: None,
            last_tick: None,
            observed: Observed::default(),
            focus_requested: false,
        }
    }

    /// Registers this process's [`IpcBridge`] (present only for the primary
    /// instance, #11); polled once per tick.
    pub fn attach_ipc(&mut self, ipc: IpcBridge) {
        self.ipc = Some(ipc);
    }

    /// Sets a one-line startup notice (e.g. "Audio unavailable: ...") shown
    /// under the menu bar until the app is restarted.
    pub fn set_backend_notice(&mut self, notice: impl Into<String>) {
        self.backend_notice = Some(notice.into());
    }

    /// The startup notice, if any.
    #[must_use]
    pub fn backend_notice(&self) -> Option<&str> {
        self.backend_notice.as_deref()
    }

    /// Queues a command to be applied by the next [`Shell::tick`].
    pub fn dispatch(&mut self, cmd: Command) {
        self.state.push(cmd);
    }

    /// Whether IPC asked the window to come forward since the last call.
    /// Consumes the request.
    pub fn take_focus_request(&mut self) -> bool {
        std::mem::take(&mut self.focus_requested)
    }

    /// Applies one request from the CLI or from a secondary instance (#11):
    /// stop and replace the queue with the message's files (or append them),
    /// resolved against its working directory.
    pub fn handle_ipc_message(&mut self, message: winshell::IpcMessage) {
        self.focus_requested = true;
        let paths = ipc::resolve_paths(&message);
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

    /// Polls the backends, applies a frame's queued commands, runs the
    /// debounced config save and reports what changed plus when to wake next.
    pub fn tick(&mut self, now: Instant) -> Tick {
        let dt = self
            .last_tick
            .map_or(Duration::ZERO, |last| now.saturating_duration_since(last));
        self.last_tick = Some(now);

        // Apply background updates (new library snapshots, scan progress,
        // recorded plays) before any view reads the data.
        self.library.tick();
        // Route finished tag edits to the editor that requested them (#172),
        // so it can clear its pending state or show the per-file error.
        let tag_edit_results = self.library.take_tag_edit_results();
        tag_editor::deliver(&mut self.state.tag_editor, tag_edit_results);

        self.player.tick(dt);

        // Keep both search engines in sync: the top bar's (drives the Music
        // view's live filter) and the popup's (independent, so typing in the
        // popup never changes what's filtered underneath it). Both run their
        // matching on background threads, which wake the frontend when done.
        self.search
            .tick(self.library.tracks(), &self.state.search_query);
        self.popup_search
            .tick(self.library.tracks(), &self.state.search_popup.query);

        self.poll_ipc();
        self.apply_pending();
        self.tick_config_persistence(now);

        let changes = self.diff();
        let next_wake = self.next_wake();
        Tick { changes, next_wake }
    }

    /// Writes the config one last time on exit, including the playback
    /// session (#190) that is deliberately left out of the debounced save.
    pub fn save_on_exit(&mut self) {
        let mut current = Config::capture(&self.state, self.player.as_ref());
        if self.state.resume_playback {
            current.last_played = LastPlayed::capture(self.player.as_ref());
        }
        self.write_config(current);
    }

    fn apply_pending(&mut self) {
        let commands = std::mem::take(&mut self.state.pending);
        for cmd in &commands {
            self.state.apply_local(cmd);
            commands::apply_player_command(self.player.as_mut(), self.library.as_ref(), cmd);
        }
        commands::apply_library_commands(self.library.as_mut(), &mut self.state, &commands);
    }

    fn poll_ipc(&mut self) {
        if let Some(message) = self.ipc.as_ref().and_then(IpcBridge::try_recv) {
            self.handle_ipc_message(message);
        }
    }

    /// Config persistence policy from #8: write 2 s after the last change
    /// (plus once more on exit via [`Shell::save_on_exit`]). No-op when
    /// persistence is disabled.
    fn tick_config_persistence(&mut self, now: Instant) {
        if self.config_path.is_none() {
            return;
        }
        let current = Config::capture(&self.state, self.player.as_ref());
        if current == self.saved {
            self.dirty_since = None;
            return;
        }
        let dirty_since = *self.dirty_since.get_or_insert(now);
        if now.saturating_duration_since(dirty_since) >= SAVE_DEBOUNCE {
            self.write_config(current);
        }
    }

    fn write_config(&mut self, config: Config) {
        let Some(path) = self.config_path.clone() else {
            return;
        };
        match config::save(&path, &config) {
            Ok(()) => {
                self.saved = config;
                self.dirty_since = None;
            }
            Err(err) => {
                warn!(%err, path = %path.display(), "could not save config");
            }
        }
    }

    /// Repaint policy (#6, #25): while something is playing, wake on a coarse
    /// one-second tick (or at [`FRAME_INTERVAL`] while the opt-in visualizer
    /// animates); otherwise only while a tag edit is in flight. `None` means
    /// the frontend may stay idle until woken by a [`Waker`](crate::waker::Waker).
    fn next_wake(&self) -> Option<Duration> {
        if self.player.status() == PlaybackStatus::Playing {
            if self.state.visualizer_enabled && self.state.visualizer != VisualizerMode::Off {
                return Some(FRAME_INTERVAL);
            }
            return Some(Duration::from_secs(1));
        }
        if matches!(
            self.state.tag_editor.as_ref().map(|editor| &editor.status),
            Some(TagStatus::Pending)
        ) {
            return Some(TAG_EDIT_REPAINT);
        }
        None
    }

    /// Diffs the compact display state against the previous tick's copy.
    fn diff(&mut self) -> Changes {
        let current = self.observe();
        let mut changes = Changes::empty();
        if current.track_count != self.observed.track_count
            || current.scanning != self.observed.scanning
            || current.status_text != self.observed.status_text
            || current.history_len != self.observed.history_len
        {
            changes |= Changes::LIBRARY;
        }
        if current.now_playing_path != self.observed.now_playing_path
            || current.playback != self.observed.playback
        {
            changes |= Changes::NOW_PLAYING;
        }
        if current.position_ms != self.observed.position_ms {
            changes |= Changes::POSITION;
        }
        if current.queue_len != self.observed.queue_len
            || current.queue_head != self.observed.queue_head
        {
            changes |= Changes::QUEUE;
        }
        if current.search_active != self.observed.search_active
            || current.search_count != self.observed.search_count
            || current.popup_search_active != self.observed.popup_search_active
            || current.popup_search_count != self.observed.popup_search_count
        {
            changes |= Changes::SEARCH;
        }
        if current.view != self.observed.view {
            changes |= Changes::VIEW;
        }
        if current.theme != self.observed.theme || current.accent != self.observed.accent {
            changes |= Changes::THEME;
        }
        if current.panels != self.observed.panels {
            changes |= Changes::PANELS;
        }
        self.observed = current;
        changes
    }

    fn observe(&self) -> Observed {
        Observed {
            track_count: self.library.track_count(),
            scanning: self.library.is_scanning(),
            status_text: self.library.status_text(),
            history_len: self.library.history().len(),
            now_playing_path: self.player.now_playing().map(|info| info.path.clone()),
            playback: self.player.status(),
            position_ms: self.player.position().as_millis() as u64,
            queue_len: self.player.queue().len(),
            queue_head: self.player.queue().first().map(|entry| entry.title.clone()),
            search_active: self.search.is_active(),
            search_count: self.search.match_count(),
            popup_search_active: self.popup_search.is_active(),
            popup_search_count: self.popup_search.match_count(),
            view: self.state.view,
            theme: self.state.theme,
            accent: self.state.accent,
            panels: self.state.panels,
        }
    }
}

#[cfg(test)]
mod tests;
