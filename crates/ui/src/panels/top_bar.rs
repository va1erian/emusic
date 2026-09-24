//! Top transport bar model (#104): the live transport snapshot plus the
//! intents of the prev/play-pause/stop/next buttons, the repeat and shuffle
//! toggles, the seek and volume sliders, and the search box.
//!
//! All state and display logic live here; each frontend owns the widgets and
//! maps their events to [`TopBarMsg`]. The search *text* stays in
//! [`AppState::search_query`](crate::state::AppState::search_query) — it is
//! shared with the views and the global search popup — so the search box only
//! emits [`TopBarMsg::SetSearchQuery`].

use std::time::Duration;

use crate::player_api::{PlaybackStatus, PlayerApi, RepeatMode};
use crate::state::Command;
use crate::views::Commands;

/// A user intent on the top bar.
#[derive(Debug, Clone, PartialEq)]
pub enum TopBarMsg {
    Previous,
    PlayPause,
    Stop,
    Next,
    /// Seek to an absolute position. Ignored when the player cannot seek, so a
    /// frontend needn't check [`TopBar::seek_supported`] first.
    Seek(Duration),
    /// Set the playback volume (clamped to `0.0 ..= 1.0`).
    SetVolume(f32),
    ToggleRepeat,
    ToggleShuffle,
    SetSearchQuery(String),
}

/// The top bar's transport snapshot and display helpers.
///
/// The frontend calls [`sync`](TopBar::sync) once per frame with the live
/// [`PlayerApi`], draws from the read-only accessors, and feeds its widget
/// events to [`update`](TopBar::update).
#[derive(Debug, Clone, PartialEq)]
pub struct TopBar {
    status: PlaybackStatus,
    position: Duration,
    duration: Option<Duration>,
    volume: f32,
    repeat: RepeatMode,
    shuffle: bool,
    seek_supported: bool,
    revision: u64,
}

impl Default for TopBar {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            position: Duration::ZERO,
            duration: None,
            volume: 1.0,
            repeat: RepeatMode::Off,
            shuffle: false,
            seek_supported: false,
            revision: 0,
        }
    }
}

impl TopBar {
    /// Reads the player's live transport state, bumping [`revision`] only when
    /// something the bar displays changed.
    ///
    /// [`revision`]: TopBar::revision
    pub fn sync(&mut self, player: &dyn PlayerApi) {
        let status = player.status();
        let position = player.position();
        let duration = player.duration();
        let volume = player.volume();
        let repeat = player.repeat_mode();
        let shuffle = player.shuffle();
        let seek_supported = player.seek_supported();
        if self.status == status
            && self.position == position
            && self.duration == duration
            && self.volume == volume
            && self.repeat == repeat
            && self.shuffle == shuffle
            && self.seek_supported == seek_supported
        {
            return;
        }
        self.status = status;
        self.position = position;
        self.duration = duration;
        self.volume = volume;
        self.repeat = repeat;
        self.shuffle = shuffle;
        self.seek_supported = seek_supported;
        self.revision += 1;
    }

    /// Applies one intent, queueing the shell [`Command`]s it produces.
    pub fn update(&mut self, msg: TopBarMsg, out: &mut Commands) {
        match msg {
            TopBarMsg::Previous => out.push(Command::PlayerPrevious),
            TopBarMsg::PlayPause => out.push(Command::PlayerPlayPause),
            TopBarMsg::Stop => out.push(Command::PlayerStop),
            TopBarMsg::Next => out.push(Command::PlayerNext),
            TopBarMsg::Seek(position) => {
                if self.seek_supported {
                    out.push(Command::PlayerSeek(position));
                }
            }
            TopBarMsg::SetVolume(volume) => {
                out.push(Command::PlayerSetVolume(volume.clamp(0.0, 1.0)));
            }
            TopBarMsg::ToggleRepeat => out.push(Command::PlayerToggleRepeat),
            TopBarMsg::ToggleShuffle => out.push(Command::PlayerToggleShuffle),
            TopBarMsg::SetSearchQuery(query) => out.push(Command::SetSearchQuery(query)),
        }
    }

    /// The revision counter, bumped whenever the displayed transport changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn status(&self) -> PlaybackStatus {
        self.status
    }

    /// Whether playback is running, so the frontend draws pause instead of
    /// play.
    pub fn is_playing(&self) -> bool {
        self.status == PlaybackStatus::Playing
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Whether repeat is on in any mode (the toggle reads as active).
    pub fn repeat_active(&self) -> bool {
        self.repeat != RepeatMode::Off
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// The current position in seconds, for the seek slider.
    pub fn position_secs(&self) -> f64 {
        self.position.as_secs_f64()
    }

    /// The track's total length in seconds, when known.
    pub fn duration_secs(&self) -> Option<f64> {
        self.duration.map(|duration| duration.as_secs_f64())
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Whether the current track can be seeked; when false the seek slider is
    /// drawn disabled.
    pub fn seek_supported(&self) -> bool {
        self.seek_supported
    }

    /// The elapsed-time label (`m:ss`).
    pub fn elapsed_text(&self) -> String {
        format_time(self.position_secs())
    }

    /// The total-time label (`m:ss`), or `None` when the length is unknown.
    pub fn total_text(&self) -> Option<String> {
        self.duration_secs().map(format_time)
    }
}

/// Formats a duration in seconds as `m:ss`, matching the other frontends.
pub fn format_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockPlayer;

    fn commands(msg: TopBarMsg, top_bar: &mut TopBar) -> Vec<Command> {
        let mut out = Commands::new();
        top_bar.update(msg, &mut out);
        out.into_vec()
    }

    #[test]
    fn sync_reads_the_player_and_tracks_revision() {
        let player = MockPlayer::default();
        let mut top_bar = TopBar::default();
        top_bar.sync(&player);
        let after_first = top_bar.revision();
        top_bar.sync(&player);
        assert_eq!(top_bar.revision(), after_first, "stable state doesn't bump");
        assert_eq!(top_bar.status(), player.status());
        assert_eq!(top_bar.volume(), player.volume());
        assert_eq!(top_bar.seek_supported(), player.seek_supported());
    }

    #[test]
    fn transport_messages_emit_their_commands() {
        let mut top_bar = TopBar::default();
        assert_eq!(
            commands(TopBarMsg::Previous, &mut top_bar),
            vec![Command::PlayerPrevious]
        );
        assert_eq!(
            commands(TopBarMsg::PlayPause, &mut top_bar),
            vec![Command::PlayerPlayPause]
        );
        assert_eq!(
            commands(TopBarMsg::Stop, &mut top_bar),
            vec![Command::PlayerStop]
        );
        assert_eq!(
            commands(TopBarMsg::Next, &mut top_bar),
            vec![Command::PlayerNext]
        );
        assert_eq!(
            commands(TopBarMsg::ToggleRepeat, &mut top_bar),
            vec![Command::PlayerToggleRepeat]
        );
        assert_eq!(
            commands(TopBarMsg::ToggleShuffle, &mut top_bar),
            vec![Command::PlayerToggleShuffle]
        );
        assert_eq!(
            commands(TopBarMsg::SetSearchQuery("a".into()), &mut top_bar),
            vec![Command::SetSearchQuery("a".into())]
        );
    }

    #[test]
    fn seek_is_ignored_when_unsupported() {
        let mut top_bar = TopBar::default();
        assert!(!top_bar.seek_supported());
        assert!(commands(TopBarMsg::Seek(Duration::from_secs(5)), &mut top_bar).is_empty());

        let mut out = Commands::new();
        top_bar.seek_supported = true;
        top_bar.update(TopBarMsg::Seek(Duration::from_secs(5)), &mut out);
        assert_eq!(
            out.into_vec(),
            vec![Command::PlayerSeek(Duration::from_secs(5))]
        );
    }

    #[test]
    fn volume_is_clamped() {
        let mut top_bar = TopBar::default();
        assert_eq!(
            commands(TopBarMsg::SetVolume(2.0), &mut top_bar),
            vec![Command::PlayerSetVolume(1.0)]
        );
        assert_eq!(
            commands(TopBarMsg::SetVolume(-0.5), &mut top_bar),
            vec![Command::PlayerSetVolume(0.0)]
        );
    }

    #[test]
    fn labels_format_like_the_frontends() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(83.9), "1:23");
        assert_eq!(format_time(-4.0), "0:00");

        let mut top_bar = TopBar::default();
        assert_eq!(top_bar.elapsed_text(), "0:00");
        assert_eq!(top_bar.total_text(), None);
        top_bar.position = Duration::from_secs(5);
        top_bar.duration = Some(Duration::from_secs(125));
        assert_eq!(top_bar.elapsed_text(), "0:05");
        assert_eq!(top_bar.total_text().as_deref(), Some("2:05"));
    }
}
