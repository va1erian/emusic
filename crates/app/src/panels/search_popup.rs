//! Global search popup (Ctrl+Shift+F / Ctrl+K, #22): a small overlay with
//! its own query box and grouped, keyboard-navigable results (top artists,
//! albums and tracks).
//!
//! Artist/album matching is a plain normalized-substring search over the
//! (small) artist/album lists; the track section reuses
//! [`crate::search::SearchEngine`]'s already-computed match set so the
//! popup never re-scans the full library itself.

use eframe::egui;
use emusic_search::normalize_text;

use crate::library_api::LibraryDataSource;
use crate::search::{QUERY_HELP, SearchEngine};
use crate::state::{AppState, Command, SearchPopupItem, View};

/// Results shown per section.
const SECTION_LIMIT: usize = 5;

pub fn show(
    ctx: &egui::Context,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    search: &SearchEngine,
) {
    if !state.search_popup.open {
        return;
    }

    let items = collect_items(library, search, &state.search_popup.query);
    if items.is_empty() {
        state.search_popup.selected = 0;
    } else {
        state.search_popup.selected = state.search_popup.selected.min(items.len() - 1);
    }

    let (up, down, enter, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if down && !items.is_empty() {
        state.search_popup.selected = (state.search_popup.selected + 1) % items.len();
    }
    if up && !items.is_empty() {
        state.search_popup.selected = (state.search_popup.selected + items.len() - 1) % items.len();
    }
    if esc {
        state.search_popup.close();
        return;
    }
    let mut activate = enter && !items.is_empty();

    let mut open = true;
    egui::Window::new("Search")
        .id(egui::Id::new("global_search_popup"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
        .fixed_size(egui::vec2(460.0, 420.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut state.search_popup.query)
                        .hint_text("Search artists, albums, tracks...")
                        .desired_width(f32::INFINITY),
                );
                response.request_focus();
                ui.label("ℹ").on_hover_text(QUERY_HELP);
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                if items.is_empty() {
                    ui.weak(if state.search_popup.query.is_empty() {
                        "Type to search your library."
                    } else {
                        "No matches."
                    });
                    return;
                }

                let mut clicked = None;
                let mut last_section = None;
                for (i, row) in items.iter().enumerate() {
                    let section = section_label(&row.item);
                    if last_section != Some(section) {
                        ui.label(egui::RichText::new(section).small().weak());
                        last_section = Some(section);
                    }
                    let selected = i == state.search_popup.selected;
                    let response = ui.selectable_label(selected, &row.label);
                    if selected {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.clicked() {
                        clicked = Some(i);
                    }
                }
                if let Some(i) = clicked {
                    state.search_popup.selected = i;
                    activate = true;
                }
            });
        });

    if !open {
        state.search_popup.close();
        return;
    }

    if activate && let Some(row) = items.get(state.search_popup.selected) {
        activate_item(state, &row.item);
    }
}

/// One flattened, rendered popup row: the item itself plus its precomputed
/// display label (built in the same pass that finds it, so rendering never
/// has to re-scan the library to look a track back up by id).
struct Row {
    item: SearchPopupItem,
    label: String,
}

/// Builds the flattened, capped result list shown in the popup: up to
/// [`SECTION_LIMIT`] artists, then albums, then tracks.
fn collect_items(library: &dyn LibraryDataSource, search: &SearchEngine, query: &str) -> Vec<Row> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let needle = normalize_text(query);

    let mut rows: Vec<Row> = library
        .artists()
        .iter()
        .filter(|a| normalize_text(&a.name).contains(&needle))
        .take(SECTION_LIMIT)
        .map(|a| Row {
            item: SearchPopupItem::Artist(a.name.clone()),
            label: a.name.clone(),
        })
        .collect();

    rows.extend(
        library
            .albums()
            .iter()
            .filter(|a| normalize_text(&a.name).contains(&needle))
            .take(SECTION_LIMIT)
            .map(|a| Row {
                item: SearchPopupItem::Album {
                    name: a.name.clone(),
                    artist: a.artist.clone(),
                },
                label: format!("{} — {}", a.name, a.artist),
            }),
    );

    rows.extend(
        library
            .tracks()
            .iter()
            .filter(|t| search.is_match(t.id))
            .take(SECTION_LIMIT)
            .map(|t| Row {
                item: SearchPopupItem::Track(t.id),
                label: format!("{} — {}", t.title, t.artist),
            }),
    );

    rows
}

fn section_label(item: &SearchPopupItem) -> &'static str {
    match item {
        SearchPopupItem::Artist(_) => "ARTISTS",
        SearchPopupItem::Album { .. } => "ALBUMS",
        SearchPopupItem::Track(_) => "TRACKS",
    }
}

/// Enter or a click on an item: tracks play directly; artists/albums
/// navigate to their view and seed the top-bar search box with the name so
/// the destination view is filtered down to the picked item.
fn activate_item(state: &mut AppState, item: &SearchPopupItem) {
    match item {
        SearchPopupItem::Artist(name) => {
            state.push(Command::SetView(View::Artists));
            state.push(Command::SetSearchQuery(name.clone()));
        }
        SearchPopupItem::Album { name, .. } => {
            state.push(Command::SetView(View::Albums));
            state.push(Command::SetSearchQuery(name.clone()));
        }
        SearchPopupItem::Track(id) => {
            state.push(Command::PlayTrack(*id));
        }
    }
    state.search_popup.close();
}
