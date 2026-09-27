#![forbid(unsafe_code)]

//! Shared `souvlaki` media-controls wrapper for the OS "Now Playing" overlay.
//!
//! Windows (SMTC) and macOS (the Now Playing info centre and hardware media
//! keys) publish the current track and playback state through the same
//! `souvlaki` API; only the [`PlatformConfig`] differs. This module owns the
//! shared logic and turns the overlay's transport events back into portable
//! [`ShellAction`]s. It degrades to a no-op (logged) when the platform call
//! fails, so playback never depends on the integration.

use std::path::Path;
use std::sync::mpsc::{self, Receiver};

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::shell::{NowPlaying, ShellAction};

/// The OS media controls plus the transport events they produced.
pub(crate) struct Media {
    controls: Option<MediaControls>,
    events: Option<Receiver<MediaControlEvent>>,
    /// The metadata last published, so a redundant update is skipped.
    published_metadata: Option<Option<Published>>,
    /// The playback key last published (status, whole-second position).
    published_playback: Option<PlaybackKey>,
}

/// The track identity last published: path plus its resolved cover URL.
type Published = (String, Option<String>);

/// A compact playback snapshot for change detection.
#[derive(Clone, Copy, PartialEq, Eq)]
struct PlaybackKey {
    playing: bool,
    position_secs: u64,
    has_duration: bool,
}

impl Media {
    /// Creates the media controls for `config`, or a disabled instance when
    /// there is no usable config or the platform call fails. `source` names
    /// the platform service in the log lines ("SMTC", "Now Playing").
    pub(crate) fn new(config: Option<PlatformConfig<'_>>, source: &str) -> Media {
        let Some(config) = config else {
            return Media::disabled("no window handle", source);
        };
        let mut controls = match MediaControls::new(config) {
            Ok(controls) => controls,
            Err(err) => return Media::disabled(&err.to_string(), source),
        };
        let (tx, rx) = mpsc::channel();
        if let Err(err) = controls.attach(move |event| {
            // The receiver lives until shutdown; a failed send then just means
            // nobody is listening.
            let _ = tx.send(event);
        }) {
            return Media::disabled(&err.to_string(), source);
        }
        tracing::info!(source, "OS media controls active");
        Media {
            controls: Some(controls),
            events: Some(rx),
            published_metadata: None,
            published_playback: None,
        }
    }

    fn disabled(reason: &str, source: &str) -> Media {
        tracing::debug!(reason, source, "OS media controls unavailable; disabled");
        Media {
            controls: None,
            events: None,
            published_metadata: None,
            published_playback: None,
        }
    }

    /// Publishes `meta` to the overlay. `None` clears the metadata.
    pub(crate) fn set_now_playing(&mut self, meta: Option<&NowPlaying>) {
        if self.controls.is_none() {
            return;
        }
        let desired = meta.map(|meta| {
            let path = meta.path.clone().unwrap_or_default();
            (path.clone(), cover_url(Path::new(&path)))
        });
        if self.published_metadata.as_ref() != Some(&desired) {
            let (title, artist, album, duration, cover) = match meta {
                Some(meta) => (
                    Some(meta.title.clone()),
                    non_empty(&meta.artist),
                    non_empty(&meta.album),
                    meta.duration.filter(|duration| !duration.is_zero()),
                    desired.as_ref().and_then(|(_, url)| url.as_deref()),
                ),
                None => (Some(String::new()), None, None, None, None),
            };
            let metadata = MediaMetadata {
                title: title.as_deref(),
                artist: artist.as_deref(),
                album: album.as_deref(),
                cover_url: cover,
                duration,
            };
            if let Some(controls) = self.controls.as_mut()
                && let Err(err) = controls.set_metadata(metadata)
            {
                tracing::debug!(%err, "could not update media metadata");
            }
            self.published_metadata = Some(desired);
        }

        let Some(meta) = meta else {
            self.set_playback(MediaPlayback::Stopped, None);
            return;
        };
        let progress = meta
            .duration
            .filter(|duration| !duration.is_zero())
            .map(|_| MediaPosition(meta.position));
        let playback = if meta.playing {
            MediaPlayback::Playing { progress }
        } else {
            MediaPlayback::Paused { progress }
        };
        self.set_playback(playback, progress);
    }

    fn set_playback(&mut self, playback: MediaPlayback, progress: Option<MediaPosition>) {
        let key = PlaybackKey {
            playing: matches!(playback, MediaPlayback::Playing { .. }),
            position_secs: progress.map_or(0, |position| position.0.as_secs()),
            has_duration: progress.is_some(),
        };
        if self.published_playback == Some(key) {
            return;
        }
        if let Some(controls) = self.controls.as_mut()
            && let Err(err) = controls.set_playback(playback)
        {
            tracing::debug!(%err, "could not update media playback state");
        }
        self.published_playback = Some(key);
    }

    /// Drains the OS transport events, mapping each to a portable action.
    pub(crate) fn drain(&mut self, playing: bool) -> Vec<ShellAction> {
        let Some(events) = &self.events else {
            return Vec::new();
        };
        let mut actions = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let Some(action) = transport_action(event, playing) {
                actions.push(action);
            }
        }
        actions
    }
}

/// Maps one OS media event to the transport action it stands for.
fn transport_action(event: MediaControlEvent, playing: bool) -> Option<ShellAction> {
    match event {
        MediaControlEvent::Play if !playing => Some(ShellAction::PlayPause),
        MediaControlEvent::Pause if playing => Some(ShellAction::PlayPause),
        MediaControlEvent::Toggle => Some(ShellAction::PlayPause),
        MediaControlEvent::Next => Some(ShellAction::Next),
        MediaControlEvent::Previous => Some(ShellAction::Previous),
        MediaControlEvent::Stop => Some(ShellAction::Stop),
        MediaControlEvent::SetPosition(position) => Some(ShellAction::Seek(position.0)),
        // Relative seeks and the remaining events have no shell equivalent.
        _ => None,
    }
}

/// A non-empty owned copy of `value`, for the overlay.
fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

/// A `file://` URL for cover art next to `path`, the form the overlay wants.
fn cover_url(path: &Path) -> Option<String> {
    let dir = path.parent()?;
    for name in ["cover", "folder", "front"] {
        for ext in ["jpg", "jpeg", "png"] {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.is_file() {
                return Some(format!("file://{}", candidate.display()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn disabled_without_a_config() {
        let mut media = Media::new(None, "test");
        media.set_now_playing(None);
        assert!(media.drain(false).is_empty());
    }

    #[test]
    fn play_and_pause_only_toggle_when_they_change_state() {
        assert_eq!(
            transport_action(MediaControlEvent::Play, false),
            Some(ShellAction::PlayPause)
        );
        assert_eq!(transport_action(MediaControlEvent::Play, true), None);
        assert_eq!(
            transport_action(MediaControlEvent::Pause, true),
            Some(ShellAction::PlayPause)
        );
        assert_eq!(transport_action(MediaControlEvent::Pause, false), None);
    }

    #[test]
    fn transport_events_map_to_actions() {
        assert_eq!(
            transport_action(MediaControlEvent::Next, false),
            Some(ShellAction::Next)
        );
        assert_eq!(
            transport_action(MediaControlEvent::Previous, false),
            Some(ShellAction::Previous)
        );
        assert_eq!(
            transport_action(
                MediaControlEvent::SetPosition(MediaPosition(Duration::from_secs(3))),
                false
            ),
            Some(ShellAction::Seek(Duration::from_secs(3)))
        );
    }
}
