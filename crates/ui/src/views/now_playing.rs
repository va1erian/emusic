//! Now-playing view model (#7, #97, #103): the right-hand panel and the
//! Now Playing view's display data and intents.
//!
//! The model turns the player/library snapshot into toolkit-agnostic display
//! data: metadata label/value rows, tracker-module info, the queue preview,
//! the progress bar fraction and the artwork request. User intents (queue
//! jump/remove, star, edit tags, properties, artist/album navigation) arrive
//! as [`NowPlayingMsg`]; `update` applies them and queues [`Command`]s. The
//! artwork cache itself is frontend-owned (it holds GPU/OS handles, #96).

use std::time::Duration;

use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::{ModuleInfo, NowPlayingInfo, PlayerApi, QueueEntry};
use crate::state::Command;
use crate::views::Commands;

/// Upcoming queue entries shown in the panel preview. Capped: the truncation
/// lives here so every frontend shows the same preview and count.
pub const QUEUE_PREVIEW_LIMIT: usize = 20;

/// Maximum length (in bytes) of a path shown in the metadata footer before it
/// is abbreviated from the front.
const PATH_MAX: usize = 48;

/// One queue row in the preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRow {
    /// 1-based position in the preview.
    pub number: usize,
    /// The entry's index in the full queue, for jump/remove commands.
    pub index: usize,
    pub title: String,
    pub artist: String,
}

/// One metadata field: a strong label and its value (empty values render as
/// an em dash, matching the Properties dialog).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataField {
    pub label: &'static str,
    pub value: String,
}

/// The artwork request for the frontend's cache: the playing path plus the
/// library track's directory to fall back to for a folder image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArtworkRequest {
    /// The playing path to load artwork for; empty when nothing is playing.
    pub path: String,
    /// Directory to search for a folder image when `path` has none.
    pub fallback_dir: Option<String>,
}

/// Tracker-module display data, ready to render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleView {
    pub format: String,
    pub name: String,
    pub channels: u32,
    pub orders: u32,
    pub current_order: u32,
    pub current_row: u32,
    pub message: String,
    pub instruments: Vec<String>,
    pub samples: Vec<String>,
}

impl ModuleView {
    /// The collapsed "Details" toggle label.
    pub fn details_label(&self) -> String {
        format!(
            "Details ({} instruments, {} samples)",
            self.instruments.len(),
            self.samples.len()
        )
    }

    /// The `Order NN / Row NNN` line.
    pub fn order_row_text(&self) -> String {
        format!(
            "Order {:02} / Row {:03}",
            self.current_order, self.current_row
        )
    }

    /// The `name · N channels · N orders` line.
    pub fn summary_text(&self) -> String {
        format!(
            "{} · {} channels · {} orders",
            self.name, self.channels, self.orders
        )
    }
}

/// The metadata details line: codec/format, bitrate, sample rate, bit depth,
/// channels and duration.
#[must_use]
pub fn metadata_details(track: &TrackInfo) -> String {
    let mut parts = Vec::new();
    if !track.codec.is_empty() {
        parts.push(track.codec.clone());
    } else if !track.format.is_empty() {
        parts.push(track.format.to_uppercase());
    }
    if let Some(kbps) = track.bitrate {
        parts.push(format!("{kbps} kbps"));
    }
    if let Some(rate) = track.sample_rate {
        parts.push(format!("{rate} Hz"));
    }
    if let Some(bits) = track.bit_depth {
        parts.push(format!("{bits}-bit"));
    }
    if let Some(ch) = track.channels {
        parts.push(format!("{ch}ch"));
    }
    parts.push(format_duration(track.duration));
    parts.join("  ·  ")
}

/// Format a duration as `m:ss`.
#[must_use]
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Keep the last ~`PATH_MAX` characters of a path, prefixed with an ellipsis.
/// Cuts on a `char` boundary so multi-byte filenames (e.g. Japanese) don't
/// panic.
#[must_use]
pub fn truncate_path(path: &str) -> String {
    if path.len() <= PATH_MAX {
        return path.to_string();
    }
    let start = path.floor_char_boundary(path.len() - PATH_MAX + 3);
    format!("...{}", &path[start..])
}

/// A user intent on the now-playing surfaces.
#[derive(Debug, Clone, PartialEq)]
pub enum NowPlayingMsg {
    /// Jump to a queue entry (double-click / Enter).
    QueueJump(usize),
    /// Remove a queue entry.
    QueueRemove(usize),
    /// Flip whether the playing track is starred.
    ToggleStar(u64),
    /// Open the tag editor for the playing track.
    EditTags(u64),
    /// Open the Properties dialog for the playing track.
    ShowProperties(Box<TrackInfo>),
    /// Jump to the Artists view for `name`.
    GoToArtist(String),
    /// Jump to the Albums view for `name`/`artist`.
    GoToAlbum { name: String, artist: String },
}

/// The now-playing panel's model: the display data for the current track
/// plus the persistent Properties dialog state.
#[derive(Debug, Default)]
pub struct NowPlayingView {
    /// The track whose Properties dialog is open, if any. Rendered by the
    /// frontend so it works whichever view is active.
    pub properties: Option<TrackInfo>,
    /// The playing track, resolved to its library entry when possible.
    current: Option<Current>,
    /// Whether a track is loaded at all (in the library or not).
    playing: bool,
    /// Upcoming queue preview rows.
    queue: Vec<QueueRow>,
    /// Total number of queue entries (may exceed the preview).
    queue_len: usize,
    /// The artwork request for the frontend's cache.
    artwork: ArtworkRequest,
    /// Progress bar state: the label and fraction, or `None` for an
    /// indeterminate bar.
    progress: Progress,
    /// The revision counter, bumped whenever the displayed state changes.
    revision: u64,
}

/// The currently playing track, with its library metadata when available.
#[derive(Debug, Default)]
struct Current {
    np: NowPlayingInfo,
    track: Option<TrackInfo>,
    /// Release year of the track's album, looked up from the album list.
    album_year: Option<u32>,
    module: Option<ModuleInfo>,
}

/// Progress bar display data.
#[derive(Debug, Clone, PartialEq)]
enum Progress {
    /// A known `position / duration`, shown as a filling bar.
    Known { fraction: f32, text: String },
    /// Elapsed time only, shown as an animated indeterminate bar.
    Indeterminate { text: String },
}

impl Default for Progress {
    fn default() -> Self {
        Progress::Indeterminate {
            text: "0:00".to_string(),
        }
    }
}

impl NowPlayingView {
    /// Rebuilds the display data from the player and library.
    pub fn refresh(&mut self, player: &dyn PlayerApi, library: &dyn LibraryDataSource) {
        let np = player.now_playing().cloned();
        let track = np
            .as_ref()
            .and_then(|info| library.track_by_path(&info.path).cloned());
        let album_year = track.as_ref().and_then(|track| album_year(track, library));
        let module = player.module_info().cloned();

        let playing = np.is_some();
        let path = np
            .as_ref()
            .map(|info| info.path.clone())
            .unwrap_or_default();
        let fallback_dir = track
            .as_ref()
            .and_then(|t| std::path::Path::new(&t.path).parent())
            .map(|p| p.to_string_lossy().into_owned());
        let artwork = ArtworkRequest { path, fallback_dir };

        let queue_len = player.queue().len();
        let queue = queue_rows(player.queue());

        let progress = progress(player, np.as_ref());

        let current = np.map(|np| Current {
            np,
            track,
            album_year,
            module,
        });
        // Compare the parts that affect what's drawn (cheap keys, not whole
        // tracks: `TrackInfo` isn't `PartialEq` and comparing every tag each
        // frame would be wasteful).
        let signature = |c: &Option<Current>| {
            c.as_ref().map(|c| {
                (
                    c.np.path.clone(),
                    c.module
                        .as_ref()
                        .map(|m| (m.current_order, m.current_row, m.message.clone())),
                    c.track.as_ref().map(|t| (t.id, t.starred)),
                )
            })
        };
        let changed = signature(&current) != signature(&self.current)
            || playing != self.playing
            || queue != self.queue
            || queue_len != self.queue_len
            || artwork != self.artwork
            || progress != self.progress;
        self.current = current;
        self.playing = playing;
        self.queue = queue;
        self.queue_len = queue_len;
        self.artwork = artwork;
        self.progress = progress;
        if changed {
            self.revision += 1;
        }
    }

    /// The track loaded in the player, if any.
    pub fn now_playing(&self) -> Option<&NowPlayingInfo> {
        self.current.as_ref().map(|c| &c.np)
    }

    /// The playing track's library metadata, if it exists in the library.
    pub fn track(&self) -> Option<&TrackInfo> {
        self.current.as_ref().and_then(|c| c.track.as_ref())
    }

    /// Whether anything is loaded in the player.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// The tracker-module display data, if the current track is a module.
    pub fn module(&self) -> Option<ModuleView> {
        self.current
            .as_ref()
            .and_then(|c| c.module.as_ref())
            .map(module_view)
    }

    /// The label/value rows for the playing track's metadata (only populated
    /// when the track exists in the library).
    pub fn metadata_fields(&self) -> Vec<MetadataField> {
        let Some(track) = self.track() else {
            return Vec::new();
        };
        vec![
            field("Title", title_text(track)),
            field("Artist", artist_text(track)),
            field("Album", &track.album),
            field("Album artist", &track.album_artist),
            field("Genre", &track.genre),
            field("Year", optional(track.year)),
            field("Track", optional(track.track_no)),
            field("Disc", optional(track.disc_no)),
            field("Composer", &track.composer),
            field("Duration", format_duration(track.duration)),
            field("Format", format_text(track)),
            field("Bitrate", bitrate_text(track.bitrate)),
            field("Sample rate", sample_rate_text(track.sample_rate)),
            field("File", &track.path),
            field("Comment", &track.comment),
        ]
    }

    /// The metadata details line for the playing track.
    pub fn details_text(&self) -> Option<String> {
        self.track().map(metadata_details)
    }

    /// Release year of the playing track's album, when known.
    pub fn album_year(&self) -> Option<u32> {
        self.current.as_ref().and_then(|c| c.album_year)
    }

    /// The upcoming queue preview rows.
    pub fn queue(&self) -> &[QueueRow] {
        &self.queue
    }

    /// Total number of queue entries.
    pub fn queue_len(&self) -> usize {
        self.queue_len
    }

    /// The `N tracks` queue header.
    pub fn queue_count_text(&self) -> String {
        format!("{} tracks", self.queue_len)
    }

    /// The artwork request for the frontend's cache.
    pub fn artwork(&self) -> &ArtworkRequest {
        &self.artwork
    }

    /// The progress bar's text, e.g. `1:16 / 5:15` or `1:16`.
    pub fn progress_text(&self) -> &str {
        match &self.progress {
            Progress::Known { text, .. } | Progress::Indeterminate { text } => text,
        }
    }

    /// The progress fraction, when the total duration is known.
    pub fn progress_fraction(&self) -> Option<f32> {
        match &self.progress {
            Progress::Known { fraction, .. } => Some(*fraction),
            Progress::Indeterminate { .. } => None,
        }
    }

    /// The revision counter, bumped whenever the displayed state changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies one user intent, queueing any resulting commands.
    pub fn update(&mut self, msg: NowPlayingMsg, out: &mut Commands) {
        match msg {
            NowPlayingMsg::QueueJump(index) => out.push(Command::PlayerQueueJump(index)),
            NowPlayingMsg::QueueRemove(index) => out.push(Command::PlayerQueueRemove(index)),
            NowPlayingMsg::ToggleStar(id) => out.push(Command::ToggleStarred(id)),
            NowPlayingMsg::EditTags(id) => out.push(Command::OpenTagEditor(id)),
            NowPlayingMsg::ShowProperties(track) => self.properties = Some(*track),
            NowPlayingMsg::GoToArtist(name) => out.push(Command::GoToArtist(name)),
            NowPlayingMsg::GoToAlbum { name, artist } => {
                out.push(Command::GoToAlbum { name, artist })
            }
        }
    }
}

/// Builds the queue preview rows (capped at [`QUEUE_PREVIEW_LIMIT`]).
fn queue_rows(queue: &[QueueEntry]) -> Vec<QueueRow> {
    queue
        .iter()
        .enumerate()
        .take(QUEUE_PREVIEW_LIMIT)
        .map(|(index, entry)| QueueRow {
            number: index + 1,
            index,
            title: entry.title.clone(),
            artist: entry.artist.clone(),
        })
        .collect()
}

fn progress(player: &dyn PlayerApi, np: Option<&NowPlayingInfo>) -> Progress {
    let position = player.position();
    match player.duration() {
        Some(duration) if duration.as_secs_f32() > 0.0 => {
            let fraction = (position.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
            Progress::Known {
                fraction,
                text: format!(
                    "{} / {}",
                    format_duration(position),
                    format_duration(duration)
                ),
            }
        }
        _ => {
            let _ = np;
            Progress::Indeterminate {
                text: format_duration(position),
            }
        }
    }
}

/// Release year for `track`'s album, looked up from the album list so it can
/// be shown next to the album name.
fn album_year(track: &TrackInfo, library: &dyn LibraryDataSource) -> Option<u32> {
    library
        .albums()
        .iter()
        .find(|a| a.name == track.album && (a.artist == track.artist || a.artist.is_empty()))
        .and_then(|a| a.year)
}

fn module_view(module: &ModuleInfo) -> ModuleView {
    ModuleView {
        format: module.format.clone(),
        name: module.name.clone(),
        channels: module.channels,
        orders: module.orders,
        current_order: module.current_order,
        current_row: module.current_row,
        message: module.message.clone(),
        instruments: module.instruments.clone(),
        samples: module.samples.clone(),
    }
}

fn field(label: &'static str, value: impl Into<String>) -> MetadataField {
    MetadataField {
        label,
        value: value.into(),
    }
}

fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// Placeholder shown for an empty/unknown title.
fn title_text(track: &TrackInfo) -> String {
    if track.title.is_empty() {
        "(unknown title)".to_string()
    } else {
        track.title.clone()
    }
}

/// Placeholder shown for an empty/unknown artist tag.
fn artist_text(track: &TrackInfo) -> String {
    if track.artist.is_empty() {
        "(unknown artist)".to_string()
    } else {
        track.artist.clone()
    }
}

/// `format (codec)`, e.g. `flac (FLAC)`; just the format when the codec is
/// unknown.
fn format_text(track: &TrackInfo) -> String {
    if track.codec.is_empty() {
        track.format.clone()
    } else {
        format!("{} ({})", track.format, track.codec)
    }
}

fn bitrate_text(bitrate: Option<u32>) -> String {
    bitrate.map(|b| format!("{b} kbps")).unwrap_or_default()
}

fn sample_rate_text(sample_rate: Option<u32>) -> String {
    sample_rate
        .map(|rate| format!("{rate} Hz"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MockLibrary, MockPlayer};

    #[test]
    fn truncate_path_cuts_on_a_char_boundary() {
        let path =
            "C:\\music\\情報デスクVIRTUAL - 札幌コンテンポラリー - 26 Untitled STRETCHED.ogg";
        let truncated = truncate_path(path);
        let tail = truncated
            .strip_prefix("...")
            .expect("truncated paths start with an ellipsis");
        assert!(path.ends_with(tail));
        assert!(!tail.is_empty());
    }

    #[test]
    fn truncate_path_keeps_short_paths_intact() {
        assert_eq!(truncate_path("C:\\music\\a.mp3"), "C:\\music\\a.mp3");
    }

    #[test]
    fn refresh_builds_queue_preview_and_progress_from_the_player() {
        let library = MockLibrary::new();
        let mut player = MockPlayer::default();
        player.replace_and_play(&[std::path::PathBuf::from("song.mp3")], 0);

        let mut view = NowPlayingView::default();
        view.refresh(&player, &library);

        assert!(view.is_playing());
        let source: &dyn LibraryDataSource = &library;
        let _ = source;
        assert_eq!(view.queue_len(), player.queue().len());
        assert!(view.progress_text().contains('/') || !view.progress_text().is_empty());
    }

    #[test]
    fn queue_jump_and_remove_emit_commands() {
        let mut view = NowPlayingView::default();
        let mut out = Commands::new();
        view.update(NowPlayingMsg::QueueJump(3), &mut out);
        view.update(NowPlayingMsg::QueueRemove(1), &mut out);
        assert_eq!(
            out.into_vec(),
            vec![Command::PlayerQueueJump(3), Command::PlayerQueueRemove(1)]
        );
    }

    #[test]
    fn navigation_and_edits_emit_commands() {
        let mut view = NowPlayingView::default();
        let mut out = Commands::new();
        view.update(NowPlayingMsg::ToggleStar(7), &mut out);
        view.update(NowPlayingMsg::EditTags(7), &mut out);
        view.update(NowPlayingMsg::GoToArtist("A".to_string()), &mut out);
        view.update(
            NowPlayingMsg::GoToAlbum {
                name: "N".to_string(),
                artist: "A".to_string(),
            },
            &mut out,
        );
        assert_eq!(
            out.into_vec(),
            vec![
                Command::ToggleStarred(7),
                Command::OpenTagEditor(7),
                Command::GoToArtist("A".to_string()),
                Command::GoToAlbum {
                    name: "N".to_string(),
                    artist: "A".to_string(),
                },
            ]
        );
    }

    #[test]
    fn show_properties_opens_the_dialog_without_a_command() {
        let mut view = NowPlayingView::default();
        let track = TrackInfo::default();
        let mut out = Commands::new();
        view.update(
            NowPlayingMsg::ShowProperties(Box::new(track.clone())),
            &mut out,
        );
        assert!(out.is_empty());
        assert_eq!(view.properties.as_ref().map(|t| t.id), Some(track.id));
    }
}
