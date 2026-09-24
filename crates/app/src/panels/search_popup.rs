//! egui renderer for the global search popup (Ctrl+Shift+F / Ctrl+K, #22,
//! #102): a small overlay with its own query box and grouped,
//! keyboard-navigable results.
//!
//! The result list, selection and activation live in the [`SearchPopup`]
//! model (`emusic-ui`); this module only draws and forwards input as messages.

use eframe::egui;

use emusic_ui::search::{QUERY_HELP, SearchEngine};
use emusic_ui::views::search_popup::SearchPopupMsg;
use emusic_ui::views::{Commands, Ctx};

use crate::library_api::LibraryDataSource;
use crate::state::AppState;

pub fn show(
    ctx: &egui::Context,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    search: &SearchEngine,
) {
    if !state.search_popup.is_open() {
        return;
    }

    // Rebuild the flattened result list from the library and the track
    // engine's already-computed match set.
    let cx = Ctx::with_library(&[], None, library);
    state.search_popup.refresh(&cx, |id| search.is_match(id));

    let mut out = Commands::new();
    let mut messages: Vec<SearchPopupMsg> = Vec::new();

    let (up, down, enter, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if esc {
        state.search_popup.update(SearchPopupMsg::Close, &mut out);
        state.pending.extend(out.into_vec());
        return;
    }
    if down {
        messages.push(SearchPopupMsg::MoveDown);
    }
    if up {
        messages.push(SearchPopupMsg::MoveUp);
    }
    let mut activate = enter;
    let mut open = true;

    let popup = &mut state.search_popup;
    egui::Window::new("Search")
        .id(egui::Id::new("global_search_popup"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
        .fixed_size(egui::vec2(460.0, 420.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let mut query = popup.state.query.clone();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut query)
                        .hint_text("Search artists, albums, tracks...")
                        .desired_width(f32::INFINITY),
                );
                response.request_focus();
                if query != popup.state.query {
                    messages.push(SearchPopupMsg::SetQuery(query));
                }
                ui.label("ℹ").on_hover_text(QUERY_HELP);
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let rows = popup.rows();
                if rows.is_empty() {
                    ui.weak(popup.empty_message());
                    return;
                }

                let mut last_section = None;
                for (i, row) in rows.iter().enumerate() {
                    if last_section != Some(row.section()) {
                        ui.label(egui::RichText::new(row.section()).small().weak());
                        last_section = Some(row.section());
                    }
                    let selected = i == popup.state.selected;
                    let response = ui.selectable_label(selected, &row.label);
                    if selected {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.clicked() {
                        messages.push(SearchPopupMsg::Select(i));
                        activate = true;
                    }
                }
            });
        });

    for msg in messages {
        state.search_popup.update(msg, &mut out);
    }
    if activate {
        state
            .search_popup
            .update(SearchPopupMsg::Activate, &mut out);
    }
    if !open {
        state.search_popup.update(SearchPopupMsg::Close, &mut out);
    }
    state.pending.extend(out.into_vec());
}
