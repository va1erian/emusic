//! Rendering of one virtualized column-browser pane.

use eframe::egui;

use super::selection::PaneSelection;

/// Height of one row, in pixels. Kept close to the row text's line height so
/// panes stay as dense as MusicBee's.
const ROW_HEIGHT: f32 = 17.0;

/// Point size of a row's text, a touch smaller than body text so more
/// entries fit without feeling cramped.
const ROW_FONT_SIZE: f32 = 12.0;

/// One selectable row in a pane; `value == None` is the leading "All (N)"
/// row, and an empty value is shown as "(unknown)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneEntry {
    pub value: Option<String>,
    pub count: usize,
}

impl PaneEntry {
    /// Text shown for this row, without the count.
    pub fn label(&self) -> &str {
        match self.value.as_deref() {
            None => "All",
            Some("") => "(unknown)",
            Some(value) => value,
        }
    }
}

/// Shows a pane's title and its rows: the "All (N)" row stays pinned at the
/// top while the facet rows scroll (and are virtualized, so large libraries
/// stay smooth). The name is left-aligned and the count right-aligned, like
/// MusicBee.
pub fn show(
    ui: &mut egui::Ui,
    id_salt: &str,
    title: &str,
    entries: &[PaneEntry],
    selection: &mut PaneSelection,
) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(egui::RichText::new(title).strong());

        // Rows sit flush against each other (no inter-item gap); the pane's
        // density comes from the row height alone.
        ui.spacing_mut().item_spacing.y = 0.0;

        let Some((all, facets)) = entries.split_first() else {
            return;
        };

        row(ui, all, selection);
        egui::ScrollArea::vertical()
            .id_salt(id_salt)
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, facets.len(), |ui, range| {
                for index in range {
                    row(ui, &facets[index], selection);
                }
            });
    });
}

fn row(ui: &mut egui::Ui, entry: &PaneEntry, selection: &mut PaneSelection) {
    let selected = match entry.value.as_deref() {
        None => selection.is_all(),
        Some(value) => selection.contains(value),
    };

    let width = ui.available_width();
    let response = ui.add_sized(
        [width, ROW_HEIGHT],
        egui::Button::selectable(
            selected,
            egui::RichText::new(entry.label()).size(ROW_FONT_SIZE),
        )
        .small()
        .right_text(
            egui::RichText::new(entry.count.to_string())
                .weak()
                .size(ROW_FONT_SIZE),
        ),
    );
    if response.clicked() {
        let ctrl = ui.input(|i| i.modifiers.command);
        selection.click(entry.value.as_deref(), ctrl);
    }
}
