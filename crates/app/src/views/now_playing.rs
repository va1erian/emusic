//! "Now playing" view: the larger now-playing summary (clickable artist and
//! album, technical details, a Properties link and a progress bar) above the
//! full-width up-next queue.
//!
//! The summary and queue are shared with the right-hand panel, so both
//! surfaces stay in sync.

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::panels::now_playing::{metadata, queue};
use crate::player_api::PlayerApi;
use crate::state::AppState;

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    let np = player.now_playing();
    let track = np.and_then(|info| library.track_by_path(&info.path));

    match (np, track) {
        (Some(np), Some(track)) => metadata::show(ui, np, track, library, state),
        (Some(np), None) => metadata::show_basic(ui, np),
        (None, _) => {
            ui.label(egui::RichText::new("Nothing is playing.").weak());
        }
    }

    if np.is_some() {
        ui.add_space(6.0);
        progress(ui, player);
    }

    ui.add_space(12.0);
    queue::show(ui, state, player);
}

/// Full-width progress bar with a `position / duration` caption.
fn progress(ui: &mut egui::Ui, player: &dyn PlayerApi) {
    let position = player.position();
    let duration = player.duration().unwrap_or_default();
    let fraction = if duration.as_secs_f32() > 0.0 {
        (position.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let text = format!(
        "{} / {}",
        format_duration(position),
        format_duration(duration)
    );
    ui.add(egui::ProgressBar::new(fraction).text(text));
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
