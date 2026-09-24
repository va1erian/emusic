//! Status-bar model (#104): the part texts the bottom bar shows — track /
//! search result count, total library duration, player state, the active
//! shuffle scope, and scan / auto-tag progress.
//!
//! Both frontends lay the same strings out; buttons (Cancel scan, Stop
//! shuffle) stay in each frontend and map to the usual
//! [`Command`](crate::state::Command)s.

use std::time::Duration;

use crate::library_api::LibraryDataSource;
use crate::player_api::{PlaybackStatus, PlayerApi};

/// The bottom status bar's part texts, synced from the shell state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusBar {
    result_count: String,
    total_duration: String,
    status: String,
    shuffle_scope: Option<String>,
    scan: Option<String>,
    scanning: bool,
    auto_tag: Option<String>,
    status_message: Option<String>,
    revision: u64,
}

impl StatusBar {
    /// Reads the library/player/state the status bar displays, bumping
    /// [`StatusBar::revision`] only when a part changed.
    ///
    /// `search_result_count` comes from the [`AppState`] the caller owns, so
    /// the model can be synced without borrowing the whole state.
    ///
    /// [`AppState`]: crate::state::AppState
    pub fn sync(
        &mut self,
        search_result_count: Option<usize>,
        library: &dyn LibraryDataSource,
        player: &dyn PlayerApi,
    ) {
        let result_count = match search_result_count {
            Some(matched) => format!("{matched} of {} tracks", library.track_count()),
            None => format!("{} tracks", library.track_count()),
        };
        let total_duration = format_total_duration(library.total_duration());
        let status = status_text(player.status()).to_string();
        let shuffle_scope = player.shuffle_scope().map(str::to_owned);
        let scan = library.status_text();
        let scanning = library.is_scanning();
        let auto_tag = library.auto_tag_status().map(|status| status.text);
        let status_message = player.status_message().map(str::to_owned);

        if self.result_count == result_count
            && self.total_duration == total_duration
            && self.status == status
            && self.shuffle_scope == shuffle_scope
            && self.scan == scan
            && self.scanning == scanning
            && self.auto_tag == auto_tag
            && self.status_message == status_message
        {
            return;
        }
        self.result_count = result_count;
        self.total_duration = total_duration;
        self.status = status;
        self.shuffle_scope = shuffle_scope;
        self.scan = scan;
        self.scanning = scanning;
        self.auto_tag = auto_tag;
        self.status_message = status_message;
        self.revision += 1;
    }

    /// The revision counter, bumped whenever a part text changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// "N tracks", or "N of M tracks" while the Music search box is active.
    pub fn result_count(&self) -> &str {
        &self.result_count
    }

    /// The library's total play time, e.g. `3h 12m total`.
    pub fn total_duration(&self) -> &str {
        &self.total_duration
    }

    /// The player state, e.g. `Playing`.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// The active scoped shuffle's label, if any.
    pub fn shuffle_scope(&self) -> Option<&str> {
        self.shuffle_scope.as_deref()
    }

    /// The library's scan progress line, if a scan is running.
    pub fn scan(&self) -> Option<&str> {
        self.scan.as_deref()
    }

    /// Whether a library scan is in progress (drives a Cancel affordance).
    pub fn is_scanning(&self) -> bool {
        self.scanning
    }

    /// The in-flight auto-tag lookup's progress line, if any.
    pub fn auto_tag(&self) -> Option<&str> {
        self.auto_tag.as_deref()
    }

    /// A transient player message (e.g. an unreadable file was skipped).
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }
}

/// The player-state label shown in the bar.
pub fn status_text(status: PlaybackStatus) -> &'static str {
    match status {
        PlaybackStatus::Playing => "Playing",
        PlaybackStatus::Paused => "Paused",
        PlaybackStatus::Stopped => "Ready",
    }
}

/// Formats a total play time as `Hh Mm total`.
pub fn format_total_duration(total: Duration) -> String {
    let secs = total.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    format!("{hours}h {minutes}m total")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MockLibrary, MockPlayer};

    #[test]
    fn counts_and_duration_and_status_format_like_before() {
        let library = MockLibrary::default();
        let player = MockPlayer::default();
        let mut bar = StatusBar::default();
        bar.sync(None, &library, &player);
        assert_eq!(
            bar.result_count(),
            format!("{} tracks", library.track_count())
        );
        assert_eq!(bar.status(), "Ready");
        assert_eq!(
            bar.total_duration(),
            format_total_duration(library.total_duration())
        );

        bar.sync(Some(3), &library, &player);
        assert_eq!(
            bar.result_count(),
            format!("3 of {} tracks", library.track_count())
        );
    }

    #[test]
    fn sync_bumps_revision_only_on_change() {
        let library = MockLibrary::default();
        let player = MockPlayer::default();
        let mut bar = StatusBar::default();
        bar.sync(None, &library, &player);
        let after_first = bar.revision();
        bar.sync(None, &library, &player);
        assert_eq!(bar.revision(), after_first);
        bar.sync(Some(1), &library, &player);
        assert!(bar.revision() > after_first);
    }

    #[test]
    fn total_duration_rounds_down_to_minutes() {
        assert_eq!(format_total_duration(Duration::from_secs(0)), "0h 0m total");
        assert_eq!(
            format_total_duration(Duration::from_secs(3 * 3600 + 12 * 60 + 59)),
            "3h 12m total"
        );
    }
}
