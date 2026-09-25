//! egui renderer for the "Now playing" view: the larger now-playing summary
//! (clickable artist and album, technical details and a Properties link)
//! above the full-width up-next queue.
//!
//! The summary and queue are shared with the right-hand panel (same
//! `NowPlayingView` model), so both surfaces stay in sync.

use eframe::egui;

use emusic_ui::views::Commands;
use emusic_ui::views::now_playing::NowPlayingMsg;

use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::AppState;

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    state.now_playing.refresh(player, library);
    let mut messages: Vec<NowPlayingMsg> = Vec::new();

    let view = &state.now_playing;
    match (view.now_playing(), view.track()) {
        (Some(np), Some(track)) => {
            crate::panels::now_playing::metadata::show(ui, np, track, view, &mut messages)
        }
        (Some(np), None) => crate::panels::now_playing::metadata::show_basic(ui, np),
        (None, _) => {
            ui.label(egui::RichText::new("Nothing is playing.").weak());
        }
    }

    ui.add_space(12.0);
    crate::panels::now_playing::queue::show(ui, view, &mut messages);

    let mut out = Commands::new();
    for msg in messages {
        state.now_playing.update(msg, &mut out);
    }
    state.pending.extend(out.into_vec());
}
