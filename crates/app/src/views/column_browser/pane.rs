//! Rendering of one virtualized column-browser pane.

use eframe::egui;

use emusic_ui::views::column_browser::{ColumnBrowser, ColumnBrowserMsg, FacetEntry, Pane};

use crate::state::Metrics;

/// Shows a pane's title and its rows: the "All (N)" row stays pinned at the
/// top while the facet rows scroll (and are virtualized, so large libraries
/// stay smooth). The name is left-aligned and the count right-aligned, like
/// MusicBee.
pub fn show(
    ui: &mut egui::Ui,
    id_salt: &str,
    title: &str,
    pane: Pane,
    browser: &ColumnBrowser,
    messages: &mut Vec<ColumnBrowserMsg>,
) {
    let metrics = crate::appearance::metrics();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(egui::RichText::new(title).strong());

        // Rows sit flush against each other (no inter-item gap); the pane's
        // density comes from the row height alone.
        ui.spacing_mut().item_spacing.y = 0.0;

        let entries = browser.facets().pane(pane);
        let Some((all, facets)) = entries.split_first() else {
            return;
        };

        row(ui, pane, browser, all, 0, metrics, messages);
        egui::ScrollArea::vertical()
            .id_salt(id_salt)
            .auto_shrink([false, false])
            .show_rows(ui, metrics.row_height, facets.len(), |ui, range| {
                for index in range {
                    row(
                        ui,
                        pane,
                        browser,
                        &facets[index],
                        index + 1,
                        metrics,
                        messages,
                    );
                }
            });
    });
}

/// Draws one row and records a click. `row_index` drives zebra parity: the
/// pinned "All" row is 0 and the facets continue from 1, so the stripes stay
/// aligned across the whole pane.
fn row(
    ui: &mut egui::Ui,
    pane: Pane,
    browser: &ColumnBrowser,
    entry: &FacetEntry,
    row_index: usize,
    metrics: Metrics,
    messages: &mut Vec<ColumnBrowserMsg>,
) {
    let selection = match pane {
        Pane::Genre => &browser.genres,
        Pane::Artist => &browser.artists,
        Pane::Album => &browser.albums,
    };
    let selected = match entry.value.as_deref() {
        None => selection.is_all(),
        Some(value) => selection.contains(value),
    };

    let width = ui.available_width();
    let row_rect =
        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(width, metrics.row_height));
    if crate::appearance::zebra() && row_index % 2 == 1 {
        ui.painter().rect_filled(
            row_rect,
            egui::CornerRadius::ZERO,
            ui.visuals().faint_bg_color,
        );
    }

    let response = ui.add_sized(
        [width, metrics.row_height],
        egui::Button::selectable(
            selected,
            egui::RichText::new(entry.label()).size(metrics.body),
        )
        .small()
        .right_text(
            egui::RichText::new(entry.count.to_string())
                .weak()
                .size(metrics.body),
        ),
    );
    if response.clicked() {
        let ctrl = ui.input(|i| i.modifiers.command);
        messages.push(ColumnBrowserMsg::RowClicked {
            pane,
            value: entry.value.clone(),
            ctrl,
        });
    }
}
