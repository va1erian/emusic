//! Rendering of one virtualized column-browser pane.

use eframe::egui;

use super::selection::PaneSelection;

/// Extra vertical space added to each row on top of the text height.
const ROW_PADDING: f32 = 2.0;

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

    fn text(&self) -> String {
        format!("{} ({})", self.label(), self.count)
    }
}

/// Shows a pane's title and its virtualized list of rows (only the visible
/// rows are laid out, so large libraries stay smooth).
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

        let row_height = ui.text_style_height(&egui::TextStyle::Body) + ROW_PADDING;
        egui::ScrollArea::vertical()
            .id_salt(id_salt)
            .auto_shrink([false, false])
            .show_rows(ui, row_height, entries.len(), |ui, range| {
                for index in range {
                    row(ui, row_height, &entries[index], selection);
                }
            });
    });
}

fn row(ui: &mut egui::Ui, row_height: f32, entry: &PaneEntry, selection: &mut PaneSelection) {
    let selected = match entry.value.as_deref() {
        None => selection.is_all(),
        Some(value) => selection.contains(value),
    };

    let width = ui.available_width();
    let response = ui.add_sized(
        [width, row_height],
        egui::Button::selectable(selected, entry.text()),
    );
    if response.clicked() {
        let ctrl = ui.input(|i| i.modifiers.command);
        selection.click(entry.value.as_deref(), ctrl);
    }
}
