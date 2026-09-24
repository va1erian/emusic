//! "History" view (#24): recorded plays grouped by day, newest first.
//!
//! Double-clicking a row plays that track again; each row has a remove
//! button, and the whole list can be cleared after a confirmation dialog.

mod table;

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};

use table::HistoryAction;

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    let history = library.history();

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} plays", history.len())).weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let enabled = !history.is_empty();
            if ui
                .add_enabled(enabled, egui::Button::new("Clear history"))
                .clicked()
            {
                state.history.confirm_clear = true;
            }
        });
    });
    ui.separator();

    if history.is_empty() {
        ui.weak("No plays recorded yet.");
    } else {
        let now = unix_now();
        let playing_id = currently_playing_id(library, player);
        let action = table::show(
            ui,
            "history_table",
            &mut state.history.selection,
            history,
            library,
            now,
            playing_id,
        );
        if let Some(action) = action {
            state.push(match action {
                // Context: the history list itself, newest-first, as shown
                // (#134). Repeated plays of the same track appear more than
                // once, same as the list on screen; "Play again" queues up
                // the visible history rather than the deduplicated library.
                HistoryAction::Play(track_id) => {
                    let context = history.iter().map(|entry| entry.track_id);
                    Command::play_track(track_id, context)
                }
                HistoryAction::PlayNext(id) => Command::PlayTrackNext(id),
                HistoryAction::AddToQueue(id) => Command::QueueTrack(id),
                HistoryAction::ToggleStar(id) => Command::ToggleStarred(id),
                HistoryAction::EditTags(id) => Command::OpenTagEditor(id),
                HistoryAction::Remove(id) => Command::HistoryRemove(id),
            });
        }
    }

    clear_confirmation(ui.ctx(), state);
}

/// The "clear history" confirmation modal, shown while the state's
/// `confirm_clear` flag is set.
fn clear_confirmation(ctx: &egui::Context, state: &mut AppState) {
    if !state.history.confirm_clear {
        return;
    }

    let mut clear = false;
    let mut cancel = false;
    let modal = egui::Modal::new(egui::Id::new("history_clear_confirm")).show(ctx, |ui| {
        ui.set_width(320.0);
        ui.heading("Clear play history?");
        ui.add_space(4.0);
        ui.label(
            "This removes every recorded play, including the most-played \
             rankings. Per-track play counts are kept. This cannot be undone.",
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
            if ui.button("Clear history").clicked() {
                clear = true;
            }
        });
    });

    if clear {
        state.push(Command::HistoryClear);
    }
    if clear || cancel || modal.should_close() {
        state.history.confirm_clear = false;
    }
}

fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library.track_by_path(&now_playing.path).map(|t| t.id)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
