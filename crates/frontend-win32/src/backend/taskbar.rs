//! DWM iconic taskbar thumbnail + progress bar + tooltip (#322), built on
//! [`winshell::taskbar`].
//!
//! Mirrors the current track onto the taskbar's hover card: cover art, title,
//! artist, album and `elapsed / total`, plus the taskbar progress bar and the
//! button tooltip. The card itself is rendered on demand — DWM asks for one
//! with `WM_DWMSENDICONICTHUMBNAIL` (recorded by
//! [`winshell::taskbar::msg_hook`] in `main.rs`), and the next tick renders it
//! and hands it back. `DwmInvalidateIconicBitmaps` is called when the track,
//! metadata, cover readiness, play/pause state or whole-second position
//! changes, so DWM re-requests the card.
//!
//! The cover reuses the shared artwork cache (see
//! [`crate::views::now_playing::artwork`]): a decode that finishes after the
//! card was last drawn bumps the invalidation key and the card is re-rendered.
//!
//! Like the other shell integrations, creation degrades to a logged no-op when
//! DWM/shell/COM is unavailable (or there is no window), so playback never
//! depends on the taskbar.

use std::path::Path;

use emusic_ui::player_api::{NowPlayingInfo, PlaybackStatus, PlayerApi};
use emusic_ui::views::now_playing::NowPlayingView;
use emusic_ui::waker::WakerHandle;
use tracing::{info, warn};

use winshell::taskbar::{self, Cover, Panel, ProgressState, TaskBar};

use crate::views::now_playing::artwork::{self, ArtworkCache, Win32ImageSink};

/// The custom taskbar preview. `inner` is `None` when the shell/DWM object
/// could not be created; every method then no-ops.
pub struct TaskbarPreview {
    inner: Option<TaskBar>,
    /// Cover-art cache shared with the now-playing panel's decoder.
    cache: ArtworkCache,
    /// The display state the last invalidation was based on, so DWM is only
    /// told the card changed when it did (including the ~1 Hz time text).
    applied: Option<PreviewKey>,
    /// Whether the "iconic card unavailable" warning was already logged.
    warned: bool,
}

/// The parts of the display state that change what the card shows. Compared
/// each tick; a difference triggers `DwmInvalidateIconicBitmaps`.
#[derive(Debug, PartialEq, Eq)]
struct PreviewKey {
    /// Now-playing model revision (track, metadata, rating, ...).
    revision: u64,
    path: String,
    playing: bool,
    elapsed_secs: u64,
    has_cover: bool,
}

impl TaskbarPreview {
    /// Creates the shell/DWM object for `hwnd` and the cover cache woken by
    /// `waker`.
    ///
    /// A missing window handle or an unavailable shell disables the
    /// integration (logged) rather than failing, mirroring the SMTC and
    /// thumbnail-toolbar integrations.
    pub fn new(hwnd: Option<isize>, waker: WakerHandle) -> Self {
        let cache = artwork::new_cache(waker);
        let Some(hwnd) = hwnd else {
            return Self::disabled("no window handle", cache);
        };
        match TaskBar::new(hwnd) {
            Ok(inner) => {
                if inner.iconic_enabled() {
                    info!("taskbar iconic thumbnail and progress bar active");
                } else {
                    warn!("DWM refuses an iconic taskbar thumbnail; keeping the default preview");
                }
                Self {
                    inner: Some(inner),
                    cache,
                    applied: None,
                    warned: false,
                }
            }
            Err(err) => Self::disabled(&err.to_string(), cache),
        }
    }

    /// Disabled integration: no shell object, all methods no-op.
    fn disabled(reason: &str, cache: ArtworkCache) -> Self {
        warn!(reason, "taskbar preview and progress bar unavailable");
        Self {
            inner: None,
            cache,
            applied: None,
            warned: false,
        }
    }

    /// Runs once per tick: polls the cover cache, invalidates the card when
    /// its content changed, answers a pending DWM thumbnail request and
    /// mirrors the progress bar and tooltip.
    pub fn sync(&mut self, player: &dyn PlayerApi, model: &NowPlayingView) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        // Poll the shared cache every tick: a decode may finish between model
        // changes, which the invalidation below turns into a re-render.
        let mut sink = Win32ImageSink;
        self.cache.drain(&mut sink);
        let request = model.artwork();
        let cover = if request.path.is_empty() {
            None
        } else {
            self.cache
                .get_with_fallback(
                    &mut sink,
                    &request.path,
                    request.fallback_dir.as_deref().map(Path::new),
                )
                .cloned()
                .flatten()
        };

        let status = player.status();
        let elapsed_secs = player.position().as_secs();
        let total_secs = player.duration().map(|duration| duration.as_secs());
        let texts = texts(model.now_playing(), model.track());

        // Invalidate when anything the card shows changed. The position is
        // whole seconds, so this is the ~1 Hz refresh while playing.
        let key = PreviewKey {
            revision: model.revision(),
            path: request.path.clone(),
            playing: status == PlaybackStatus::Playing,
            elapsed_secs,
            has_cover: cover.is_some(),
        };
        if self.applied.as_ref() != Some(&key) {
            if let Err(err) = inner.invalidate_iconic() {
                warn_once(
                    &mut self.warned,
                    &err,
                    "could not invalidate the taskbar thumbnail",
                );
            }
            self.applied = Some(key);
        }

        // DWM may have asked for a card since the last tick; render it now
        // that the cover cache has been polled.
        if let Some(size) = taskbar::take_thumbnail_request() {
            let cover = cover.as_deref().map(|image| Cover {
                width: image.width,
                height: image.height,
                rgba: &image.pixels,
            });
            let panel = Panel {
                title: &texts.title,
                artist: &texts.artist,
                album: &texts.album,
                elapsed_secs,
                total_secs: total_secs.unwrap_or(0),
                cover,
            };
            if let Err(err) = inner.set_iconic_thumbnail(size, &panel) {
                warn_once(
                    &mut self.warned,
                    &err,
                    "could not set the taskbar thumbnail",
                );
            }
        }

        let tooltip = tooltip(&texts);
        if let Err(err) = inner.set_tooltip(&tooltip) {
            warn_once(&mut self.warned, &err, "could not set the taskbar tooltip");
        }

        let progress = progress_state(status, total_secs);
        if let Err(err) = inner.set_progress(progress, elapsed_secs, total_secs.unwrap_or(0)) {
            warn_once(
                &mut self.warned,
                &err,
                "could not update the taskbar progress bar",
            );
        }
    }
}

/// Logs an integration failure once, so a per-tick shell failure cannot flood
/// the log.
fn warn_once(warned: &mut bool, err: &winshell::WinshellError, message: &str) {
    if !*warned {
        warn!(%err, message);
        *warned = true;
    }
}

/// The card's text lines, preferring the library's richer tags and falling
/// back to the player's display strings.
#[derive(Debug, Default, PartialEq, Eq)]
struct Texts {
    title: String,
    artist: String,
    album: String,
}

/// Text for the card: the app name when nothing is playing, and the track's
/// library tags when the library has them.
fn texts(np: Option<&NowPlayingInfo>, track: Option<&emusic_ui::library_api::TrackInfo>) -> Texts {
    let title = text(
        track.map(|t| t.title.as_str()),
        np.map(|n| n.title.as_str()),
    );
    let artist = text(
        track.map(|t| t.artist.as_str()),
        np.map(|n| n.artist.as_str()),
    );
    let album = text(
        track.map(|t| t.album.as_str()),
        np.map(|n| n.album.as_str()),
    );
    Texts {
        title: if title.is_empty() {
            "emusic".to_string()
        } else {
            title.to_string()
        },
        artist: artist.to_string(),
        album: album.to_string(),
    }
}

/// The first non-empty of the track's tag and the player's label.
fn text<'a>(from_track: Option<&'a str>, from_player: Option<&'a str>) -> &'a str {
    from_track
        .filter(|value| !value.is_empty())
        .or_else(|| from_player.filter(|value| !value.is_empty()))
        .unwrap_or("")
}

/// The taskbar button tooltip: `Title - Artist`, or the app name when nothing
/// is playing.
fn tooltip(texts: &Texts) -> String {
    match (texts.title.as_str(), texts.artist.as_str()) {
        ("emusic", _) => "emusic".to_string(),
        (title, "") => title.to_string(),
        (title, artist) => format!("{title} - {artist}"),
    }
}

/// The progress state for `status`; a track with no known length gets no
/// determinate bar.
fn progress_state(status: PlaybackStatus, total_secs: Option<u64>) -> ProgressState {
    match (status, total_secs) {
        (PlaybackStatus::Stopped, _) | (_, None) => ProgressState::None,
        (PlaybackStatus::Playing, Some(_)) => ProgressState::Normal,
        (PlaybackStatus::Paused, Some(_)) => ProgressState::Paused,
    }
}

#[cfg(test)]
mod tests {
    use emusic_ui::library_api::TrackInfo;
    use emusic_ui::mock::MockPlayer;
    use emusic_ui::waker::WakerSlot;

    use super::*;

    fn np(title: &str, artist: &str) -> NowPlayingInfo {
        NowPlayingInfo {
            title: title.to_string(),
            artist: artist.to_string(),
            ..NowPlayingInfo::default()
        }
    }

    #[test]
    fn library_tags_win_over_player_labels() {
        let track = TrackInfo {
            title: "Real Title".to_string(),
            artist: "Real Artist".to_string(),
            album: "Real Album".to_string(),
            ..TrackInfo::default()
        };
        let texts = texts(Some(&np("song", "")), Some(&track));
        assert_eq!(texts.title, "Real Title");
        assert_eq!(texts.artist, "Real Artist");
        assert_eq!(texts.album, "Real Album");
    }

    #[test]
    fn missing_tags_fall_back_and_nothing_shows_the_app_name() {
        let from_player = texts(Some(&np("song", "who")), None);
        assert_eq!(from_player.title, "song");
        assert_eq!(from_player.artist, "who");
        assert_eq!(from_player.album, "");

        let empty = texts(None, None);
        assert_eq!(empty.title, "emusic");
        assert_eq!(tooltip(&empty), "emusic");
    }

    #[test]
    fn tooltip_joins_title_and_artist() {
        let texts = Texts {
            title: "Song".to_string(),
            artist: "Who".to_string(),
            album: String::new(),
        };
        assert_eq!(tooltip(&texts), "Song - Who");
        let no_artist = Texts {
            title: "Song".to_string(),
            ..Texts::default()
        };
        assert_eq!(tooltip(&no_artist), "Song");
    }

    #[test]
    fn progress_reflects_the_transport_state() {
        assert_eq!(
            progress_state(PlaybackStatus::Playing, Some(200)),
            ProgressState::Normal
        );
        assert_eq!(
            progress_state(PlaybackStatus::Paused, Some(200)),
            ProgressState::Paused
        );
        assert_eq!(
            progress_state(PlaybackStatus::Stopped, Some(200)),
            ProgressState::None
        );
        assert_eq!(
            progress_state(PlaybackStatus::Playing, None),
            ProgressState::None,
            "an unknown length cannot show a determinate bar"
        );
    }

    #[test]
    fn preview_without_a_window_is_a_no_op() {
        let waker = WakerSlot::new();
        let mut preview = TaskbarPreview::new(None, waker.handle());
        assert!(preview.inner.is_none(), "disabled when there is no window");

        let player = MockPlayer::default();
        let model = NowPlayingView::default();
        preview.sync(&player, &model);
        assert!(
            preview.applied.is_none(),
            "the disabled path tracks nothing"
        );
    }
}
