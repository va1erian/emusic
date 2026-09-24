//! Top transport bar: prev/play-pause/stop/next, repeat, shuffle, seek
//! slider with elapsed/total time, volume slider, and a search box.
//!
//! Render-only (#104): the transport state, formatting and command mapping
//! live in [`emusic_ui::panels::top_bar::TopBar`]; this module draws it and
//! turns widget responses into its messages.

use eframe::egui;

use emusic_ui::panels::top_bar::{TopBar, TopBarMsg};
use emusic_ui::views::Commands;

use crate::icons;
use crate::player_api::PlayerApi;
use crate::search::QUERY_HELP;
use crate::state::AppState;
use crate::theme;

/// Stable [`egui::Id`] for the top-bar search box, so Ctrl+F can request
/// focus on it from anywhere in the frame.
pub fn search_box_id() -> egui::Id {
    egui::Id::new("top_bar_search_box")
}

/// Fixed size of every transport button, matching the accent-filled play
/// button's size so the row stays uniform while the icons are painted.
const TRANSPORT_BUTTON_SIZE: egui::Vec2 = egui::vec2(30.0, 30.0);

/// A transport icon: paints itself into the button's rect with the given
/// colour. See [`crate::icons`] for the shape definitions.
type TransportIcon = fn(&egui::Painter, egui::Rect, egui::Color32);

pub fn show(ui: &mut egui::Ui, state: &mut AppState, player: &dyn PlayerApi) {
    state.top_bar.sync(player);

    // Ctrl+F focuses the search box regardless of which widget currently
    // has focus. The app shell consumes the more specific Ctrl+Shift+F /
    // Ctrl+K shortcuts for the global search popup before this runs, so
    // they never fall through to this plain Ctrl+F check.
    let focus_requested = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F));
    if focus_requested {
        ui.memory_mut(|mem| mem.request_focus(search_box_id()));
    }

    // Messages produced while drawing are applied afterwards, so the body can
    // borrow the model immutably (one message per event, Elm-style).
    let mut messages: Vec<TopBarMsg> = Vec::new();
    let top_bar = &state.top_bar;
    let search_query = &state.search_query;
    egui::Panel::top("top_bar").exact_size(56.0).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.add_space(4.0);
            transport_buttons(ui, top_bar, &mut messages);

            ui.separator();
            toggle_button(
                ui,
                "🔁",
                "Repeat",
                top_bar.repeat_active(),
                TopBarMsg::ToggleRepeat,
                &mut messages,
            );
            toggle_button(
                ui,
                "🔀",
                "Shuffle",
                top_bar.shuffle(),
                TopBarMsg::ToggleShuffle,
                &mut messages,
            );
            ui.separator();

            // Lay the right-hand controls out from the right edge so the
            // volume slider and search box stay pinned there; the seek area
            // then fills whatever space is left in the middle.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                search_box(ui, search_query, &mut messages);
                ui.separator();
                volume_area(ui, top_bar, &mut messages);
                ui.separator();
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    seek_area(ui, top_bar, &mut messages);
                });
            });
        });
    });

    if !messages.is_empty() {
        let mut out = Commands::new();
        for msg in messages {
            state.top_bar.update(msg, &mut out);
        }
        state.pending.extend(out.into_vec());
    }
}

fn transport_buttons(ui: &mut egui::Ui, top_bar: &TopBar, messages: &mut Vec<TopBarMsg>) {
    if transport_button(ui, icons::previous, false, "Previous").clicked() {
        messages.push(TopBarMsg::Previous);
    }

    let (icon, tooltip) = if top_bar.is_playing() {
        (icons::pause as TransportIcon, "Pause")
    } else {
        (icons::play as TransportIcon, "Play")
    };
    if transport_button(ui, icon, true, tooltip).clicked() {
        messages.push(TopBarMsg::PlayPause);
    }

    if transport_button(ui, icons::stop, false, "Stop").clicked() {
        messages.push(TopBarMsg::Stop);
    }
    if transport_button(ui, icons::next, false, "Next").clicked() {
        messages.push(TopBarMsg::Next);
    }
}

/// A transport button with a custom-painted vector icon. It stays a regular
/// [`egui::Button`] (focusable, clickable, announced as a button) and only
/// the glyph is replaced by [`icons`] painting; the icon colour follows the
/// button's hover/active state, and `accent` fills it like before.
fn transport_button(
    ui: &mut egui::Ui,
    icon: TransportIcon,
    accent: bool,
    tooltip: &str,
) -> egui::Response {
    let mut button = egui::Button::new("").min_size(TRANSPORT_BUTTON_SIZE);
    if accent {
        button = button.fill(theme::current_accent());
    }
    let response = ui.add(button).on_hover_text(tooltip);
    let color = ui.style().interact(&response).fg_stroke.color;
    icon(ui.painter(), response.rect, color);
    response
}

fn toggle_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: &str,
    active: bool,
    msg: TopBarMsg,
    messages: &mut Vec<TopBarMsg>,
) {
    let mut button = egui::Button::new(icon);
    if active {
        button = button.fill(theme::current_accent());
    }
    if ui.add(button).on_hover_text(tooltip).clicked() {
        messages.push(msg);
    }
}

/// Seek slider plus elapsed and (when known) total time.
///
/// SID tunes and other channels whose backend ignores seeks get a visibly
/// disabled slider with a tooltip instead of a drag that silently snaps back;
/// a track with no known length (#192) shows elapsed time only, never a fake
/// total.
fn seek_area(ui: &mut egui::Ui, top_bar: &TopBar, messages: &mut Vec<TopBarMsg>) {
    let position = top_bar.position_secs();
    let total = top_bar.duration_secs();
    let seek_supported = top_bar.seek_supported();
    let elapsed = top_bar.elapsed_text();
    let total_text = top_bar.total_text();

    // Stretch the slider across the centre section: reserve the time labels
    // plus the item spacing around them and give the rest to the slider, so
    // the whole bar uses the available width. With no total there is only the
    // elapsed label to reserve.
    let spacing = ui.spacing().item_spacing.x;
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let labels_width = ui.fonts_mut(|fonts| {
        let mut width = |text: &str| {
            fonts
                .layout_no_wrap(text.to_owned(), font_id.clone(), egui::Color32::WHITE)
                .size()
                .x
        };
        let mut labels = width(&elapsed);
        if let Some(text) = &total_text {
            labels += width(text);
        }
        labels
    });
    let label_count = if total_text.is_some() { 2.0 } else { 1.0 };
    let slider_width = (ui.available_width() - labels_width - label_count * spacing).max(80.0);

    ui.label(&elapsed);
    let mut value = position;
    ui.spacing_mut().slider_width = slider_width;
    // Without a total, a nominal range keeps the (disabled) thumb from
    // collapsing; the position is still shown by the elapsed label.
    let range_end = total.map_or_else(|| position.max(1.0), |seconds| seconds.max(0.001));
    let slider = ui.add_enabled(
        seek_supported,
        egui::Slider::new(&mut value, 0.0..=range_end).show_value(false),
    );
    if seek_supported && slider.changed() {
        messages.push(TopBarMsg::Seek(std::time::Duration::from_secs_f64(value)));
    }
    if !seek_supported {
        slider.on_hover_text("Seeking isn't available for this track");
    }
    if let Some(total_text) = total_text {
        ui.label(&total_text);
    }
}

fn volume_area(ui: &mut egui::Ui, top_bar: &TopBar, messages: &mut Vec<TopBarMsg>) {
    ui.label("🔊");
    let mut volume = top_bar.volume();
    ui.spacing_mut().slider_width = 90.0;
    if ui
        .add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false))
        .changed()
    {
        messages.push(TopBarMsg::SetVolume(volume));
    }
}

fn search_box(ui: &mut egui::Ui, query_text: &str, messages: &mut Vec<TopBarMsg>) {
    ui.label("🔍").on_hover_text(QUERY_HELP);
    let mut query = query_text.to_owned();
    let response = ui.add(
        egui::TextEdit::singleline(&mut query)
            .id(search_box_id())
            .hint_text("Search library... (Ctrl+F)")
            .desired_width(200.0)
            .margin(egui::Margin {
                right: 20,
                ..egui::Margin::symmetric(4, 2)
            }),
    );
    if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        query.clear();
    }
    if !query.is_empty() && clear_button(ui, response.rect) {
        query.clear();
        ui.memory_mut(|mem| mem.request_focus(search_box_id()));
    }
    if query != query_text {
        messages.push(TopBarMsg::SetSearchQuery(query));
    }
    response.on_hover_text(QUERY_HELP);
}

/// A small "clear" button drawn over the right end of the search box.
/// Returns whether it was clicked.
fn clear_button(ui: &mut egui::Ui, field: egui::Rect) -> bool {
    let side = field.height() - 6.0;
    let rect = egui::Rect::from_center_size(
        egui::pos2(field.right() - side / 2.0 - 3.0, field.center().y),
        egui::Vec2::splat(side),
    );
    // A child ui, so the button doesn't advance the right-to-left layout cursor
    // and shove the neighbouring controls around.
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child
        .add(egui::Button::new(egui::RichText::new("✕").small().weak()).frame(false))
        .on_hover_text("Clear search")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
