//! Windows System Media Transport Controls (SMTC) integration (#26).
//!
//! Publishes the current track's metadata (title, artist, album, cover) and
//! playback state to the OS media overlay and hardware media keys through
//! `souvlaki`, and turns the overlay's transport events back into the shell's
//! existing [`Command`]s. Routing them through the command queue keeps a
//! single implementation of the transport logic (see [`crate::app::commands`])
//! instead of a second, parallel one.
//!
//! Metadata is one-way (app -> OS) for now. Everything degrades to a no-op
//! with a logged warning when SMTC cannot be initialised (no window handle,
//! no COM/WinRT, ...), so playback never depends on the OS integration.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use tracing::{info, warn};

use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::{NowPlayingInfo, PlaybackStatus, PlayerApi};
use crate::state::{AppState, Command};

/// Handle to the OS media controls, plus the transport events the OS sent
/// since the last frame.
pub struct Smtc {
    /// `None` when SMTC is unavailable; every method then no-ops.
    controls: Option<MediaControls>,
    /// Filled by the event handler installed on `controls` and drained each
    /// frame by [`Smtc::sync`]. The handler runs on a WinRT callback thread,
    /// hence the channel instead of shared state.
    events: Option<Receiver<MediaControlEvent>>,
    /// Cover-art lookup for the current track, resolved off the UI thread so
    /// a slow or network path can never stall a frame (see [`CoverLookup`]).
    cover: CoverLookup,
    /// Metadata currently on the overlay: the track path plus the cover URL
    /// published with it. `None` means no track metadata is published. Used
    /// to skip redundant `set_metadata` calls, including the second publish
    /// once the async cover lookup finishes.
    published_metadata: Option<(String, Option<String>)>,
    /// Last published `(status, whole-second position, whole-second
    /// duration)`, so the overlay's scrubber updates about once a second
    /// instead of every frame.
    published_playback: Option<(PlaybackStatus, u64, u64)>,
}

impl Smtc {
    /// Creates the OS media controls and attaches the event channel.
    ///
    /// `hwnd` is the window SMTC binds to; it is required on Windows and
    /// ignored elsewhere. Returns a disabled [`Smtc`] (logging a warning)
    /// rather than failing when the platform call does not work.
    pub fn new(hwnd: Option<*mut c_void>) -> Self {
        // souvlaki's Windows backend panics without an HWND, so guard first.
        #[cfg(target_os = "windows")]
        if hwnd.is_none() {
            return Self::disabled("no window handle");
        }

        let config = PlatformConfig {
            display_name: "emusic",
            dbus_name: "emusic",
            hwnd,
        };
        let mut controls = match MediaControls::new(config) {
            Ok(controls) => controls,
            Err(err) => return Self::disabled(&err.to_string()),
        };

        let (tx, rx) = mpsc::channel();
        if let Err(err) = controls.attach(move |event| {
            // The receiver is dropped only at shutdown; a failed send then
            // just means nobody is listening anymore.
            let _ = tx.send(event);
        }) {
            return Self::disabled(&err.to_string());
        }

        info!("SMTC media controls active");
        Self {
            controls: Some(controls),
            events: Some(rx),
            cover: CoverLookup::default(),
            published_metadata: None,
            published_playback: None,
        }
    }

    /// Disabled integration: no controls, no events, all methods no-op.
    fn disabled(reason: &str) -> Self {
        warn!(
            reason,
            "SMTC unavailable; OS media controls and media keys disabled"
        );
        Self {
            controls: None,
            events: None,
            cover: CoverLookup::default(),
            published_metadata: None,
            published_playback: None,
        }
    }

    /// Runs once per frame: drains the OS's transport events into queued
    /// [`Command`]s, then republishes metadata/playback state if changed.
    pub fn sync(
        &mut self,
        player: &dyn PlayerApi,
        library: &dyn LibraryDataSource,
        state: &mut AppState,
    ) {
        if self.controls.is_none() {
            return;
        }
        self.drain_events(player, state);
        self.publish(player, library);
    }

    fn drain_events(&mut self, player: &dyn PlayerApi, state: &mut AppState) {
        let Some(events) = &self.events else {
            return;
        };
        while let Ok(event) = events.try_recv() {
            if let Some(command) = transport_command(event, player) {
                state.push(command);
            }
        }
    }

    fn publish(&mut self, player: &dyn PlayerApi, library: &dyn LibraryDataSource) {
        if self.controls.is_none() {
            return;
        }

        // Never resolve a cover inline: `cover_url` can do several network
        // round-trips plus a `lofty` read and image decode. It runs on a
        // worker thread instead, and its result lands in `self.cover`.
        self.cover.drain();

        let now_playing = player.now_playing();
        let path = now_playing.map(|info| info.path.clone());
        if let Some(path) = &path {
            self.cover.request(path);
        }
        let desired = path.map(|path| {
            let cover = self.cover.url(&path);
            (path, cover)
        });

        if self.published_metadata != desired {
            let track = now_playing.and_then(|info| library.track_by_path(&info.path));
            let meta = metadata(now_playing, track);
            let cover = desired.as_ref().and_then(|(_, url)| url.as_deref());
            if let Some(controls) = self.controls.as_mut()
                && let Err(err) = controls.set_metadata(MediaMetadata {
                    title: meta.title.as_deref(),
                    artist: meta.artist.as_deref(),
                    album: meta.album.as_deref(),
                    cover_url: cover,
                    duration: meta.duration,
                })
            {
                warn!(%err, "could not update SMTC metadata");
            }
            self.published_metadata = desired;
        }

        let status = player.status();
        let position = player.position();
        let playback_key = (
            status,
            position.as_secs(),
            player.duration().map_or(0, |d| d.as_secs()),
        );
        if self.published_playback != Some(playback_key) {
            let playback = match status {
                PlaybackStatus::Playing => MediaPlayback::Playing {
                    progress: Some(MediaPosition(position)),
                },
                PlaybackStatus::Paused => MediaPlayback::Paused {
                    progress: Some(MediaPosition(position)),
                },
                PlaybackStatus::Stopped => MediaPlayback::Stopped,
            };
            if let Some(controls) = self.controls.as_mut()
                && let Err(err) = controls.set_playback(playback)
            {
                warn!(%err, "could not update SMTC playback state");
            }
            self.published_playback = Some(playback_key);
        }
    }
}

/// Owned metadata for the overlay, since `souvlaki` borrows for the duration
/// of one call.
struct Metadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<Duration>,
}

/// Metadata for the current track, preferring the library's richer tags and
/// falling back to the player's display strings. When nothing is playing the
/// title is blanked so the overlay does not keep showing the previous track.
fn metadata(np: Option<&NowPlayingInfo>, track: Option<&TrackInfo>) -> Metadata {
    let Some(np) = np else {
        return Metadata {
            title: Some(String::new()),
            artist: None,
            album: None,
            duration: None,
        };
    };

    let text = |from_track: Option<&str>, from_np: &str| -> Option<String> {
        let value = from_track.filter(|s| !s.is_empty()).unwrap_or(from_np);
        (!value.is_empty()).then(|| value.to_string())
    };

    Metadata {
        title: text(track.map(|t| t.title.as_str()), &np.title),
        artist: text(track.map(|t| t.artist.as_str()), &np.artist),
        album: text(track.map(|t| t.album.as_str()), &np.album),
        duration: Some(np.duration)
            .filter(|d| !d.is_zero())
            .or_else(|| track.map(|t| t.duration).filter(|d| !d.is_zero())),
    }
}

/// Maps one OS media event to the shell command that implements it.
fn transport_command(event: MediaControlEvent, player: &dyn PlayerApi) -> Option<Command> {
    // `Play`/`Pause` can arrive while already in that state; toggling then
    // would do the opposite of what the button says, so gate on the status.
    match event {
        MediaControlEvent::Play => matches!(
            player.status(),
            PlaybackStatus::Paused | PlaybackStatus::Stopped
        )
        .then_some(Command::PlayerPlayPause),
        MediaControlEvent::Pause => {
            (player.status() == PlaybackStatus::Playing).then_some(Command::PlayerPlayPause)
        }
        MediaControlEvent::Toggle => Some(Command::PlayerPlayPause),
        MediaControlEvent::Next => Some(Command::PlayerNext),
        MediaControlEvent::Previous => Some(Command::PlayerPrevious),
        MediaControlEvent::Stop => Some(Command::PlayerStop),
        MediaControlEvent::SetPosition(position) => Some(Command::PlayerSeek(position.0)),
        // Relative seeks and the remaining events have no shell equivalent
        // (or are outside v1's scope); ignore them.
        _ => None,
    }
}

/// A finished cover lookup: the track path it was for, and its `file://` URL
/// (`None` when no cover was found).
struct CoverResult {
    path: String,
    url: Option<String>,
}

/// Off-UI-thread cover lookup for the OS overlay.
///
/// Resolving a cover can hit the network (sibling-image probes plus a full
/// `lofty` read and image decode when there is no folder image), so each path
/// is looked up on a worker thread and the result is cached here.
/// [`CoverLookup::request`] starts at most one lookup per path and never
/// blocks; [`CoverLookup::drain`] collects finished lookups once per frame.
#[derive(Default)]
struct CoverLookup {
    /// Resolved covers by path; an entry with `None` means the lookup ran and
    /// found no cover.
    cache: HashMap<String, Option<String>>,
    /// Paths with a lookup currently in flight.
    loading: HashSet<String>,
    tx: Option<Sender<CoverResult>>,
    rx: Option<Receiver<CoverResult>>,
}

impl CoverLookup {
    /// The resolved `file://` cover URL for `path`, if the lookup has
    /// finished and found one. Also `None` while a lookup is in flight.
    fn url(&self, path: &str) -> Option<String> {
        self.cache.get(path).and_then(|url| url.clone())
    }

    /// Starts a lookup for `path` on a worker thread, unless one is already
    /// in flight or already cached.
    fn request(&mut self, path: &str) {
        if self.cache.contains_key(path) || !self.loading.insert(path.to_string()) {
            return;
        }
        let tx = self.sender();
        let key = path.to_string();
        let path = PathBuf::from(path);
        std::thread::spawn(move || {
            let url = cover_url(&path);
            let _ = tx.send(CoverResult { path: key, url });
        });
    }

    /// Collects finished lookups so [`CoverLookup::url`] can return them.
    fn drain(&mut self) {
        let Some(rx) = self.rx.as_ref() else {
            return;
        };
        while let Ok(result) = rx.try_recv() {
            self.loading.remove(&result.path);
            self.cache.insert(result.path, result.url);
        }
    }

    /// The result channel's sender, creating the channel on first use.
    fn sender(&mut self) -> Sender<CoverResult> {
        if let Some(tx) = &self.tx {
            return tx.clone();
        }
        let (tx, rx) = mpsc::channel();
        self.tx = Some(tx.clone());
        self.rx = Some(rx);
        tx
    }
}

/// `file://` URL for the current track's cover, the form Windows SMTC wants.
fn cover_url(path: &Path) -> Option<String> {
    let file = cover_file(path)?;
    Some(format!("file://{}", file.display()))
}

/// Finds a cover image for `path`: a sibling folder image first (no decode
/// needed), otherwise embedded art extracted once into a small disk cache.
fn cover_file(path: &Path) -> Option<PathBuf> {
    if let Some(dir) = path.parent() {
        for name in ["cover", "folder", "front"] {
            for ext in ["jpg", "jpeg", "png"] {
                let candidate = dir.join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    let cached = cover_cache_path(path)?;
    if cached.is_file() {
        return Some(cached);
    }

    let image = emusic_ui::image_cache::load_artwork(path)?;
    image::save_buffer(
        &cached,
        &image.pixels,
        image.width,
        image.height,
        image::ExtendedColorType::Rgba8,
    )
    .ok()?;
    Some(cached)
}

/// Where embedded cover art is cached, under the user's temp dir and named
/// after the source path so each track is extracted at most once.
fn cover_cache_path(path: &Path) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("emusic").join("smtc");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{:016x}.png", hash(&path.to_string_lossy()))))
}

fn hash(value: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests;
