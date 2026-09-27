//! File -> Database info (#275): the library counts and database file details
//! from [`emusic_ui::panels::database_info`], with a "Rescan everything"
//! button. Shown as a modal window; it answers with what the user chose.

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::panels::database_info::fields;
use win32ui::prelude::*;
use win32ui::{WindowSpec, dip, row};

/// Width of the field-name column, in design units.
const LABEL_WIDTH: f32 = 120.0;
/// Height of one field row, in design units.
const ROW_HEIGHT: f32 = 24.0;
/// The dialog's client size is fitted to its content, in design units.
const MARGIN: f32 = 16.0;
/// Approximate width of one value character, in design units.
const CHAR_WIDTH: f32 = 7.0;
const BASE_HEIGHT: f32 = 90.0;

/// What the user chose in the dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DatabaseInfoChoice {
    /// Rescan every library folder.
    Rescan,
    /// Just close the dialog.
    Close,
}

enum Msg {
    Rescan,
    Close,
}

struct DatabaseInfoDialog {
    // Held so the controls outlive the layout that positions them.
    _labels: Vec<Label>,
    _rescan: Button<Msg>,
    _close: Button<Msg>,
}

impl DatabaseInfoDialog {
    fn new(ui: &mut Ui<Msg>, fields: &[(&'static str, String)], scanning: bool) -> Self {
        let mut labels = Vec::new();
        let mut layout = Layout::column()
            .spacing(dip(4.0))
            .margins(Insets::all(dip(MARGIN)));
        for (name, value) in fields {
            let name = Label::new(ui, Rect::default(), name).expect("create field name label");
            let value = Label::new(ui, Rect::default(), value).expect("create field value label");
            layout = layout
                .item(row![name.width(dip(LABEL_WIDTH)), value.fill(1)].height(dip(ROW_HEIGHT)));
            labels.push(name);
            labels.push(value);
        }

        let rescan_text = if scanning {
            "Scanning..."
        } else {
            "Rescan everything"
        };
        let rescan = Button::new(ui, rescan_text)
            .expect("create rescan button")
            .on_click(|| Some(Msg::Rescan));
        rescan.set_enabled(!scanning);
        let close = Button::new(ui, "Close")
            .expect("create close button")
            .on_click(|| Some(Msg::Close));

        layout = layout.item(
            row![rescan.width(dip(150.0)), close.width(dip(90.0))]
                .spacing(dip(8.0))
                .height(dip(28.0)),
        );
        ui.set_layout(layout);
        Self {
            _labels: labels,
            _rescan: rescan,
            _close: close,
        }
    }
}

impl App for DatabaseInfoDialog {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        ui.close_with_result(match msg {
            Msg::Rescan => DatabaseInfoChoice::Rescan,
            Msg::Close => DatabaseInfoChoice::Close,
        });
    }
}

/// Shows the modal dialog over `ui`'s window and returns the user's choice
/// (`None` when it was dismissed with the window's close button).
pub fn show<M: 'static>(ui: &Ui<M>, library: &dyn LibraryDataSource) -> Option<DatabaseInfoChoice> {
    let fields = fields(library);
    let scanning = library.is_scanning();
    let longest = fields
        .iter()
        .map(|(_, value)| value.chars().count())
        .max()
        .unwrap_or(0);
    let width = 2.0 * MARGIN + LABEL_WIDTH + longest as f32 * CHAR_WIDTH;
    let height = BASE_HEIGHT + ROW_HEIGHT * fields.len() as f32;
    ui.open_modal(
        WindowSpec::new("Database info").size(dip(width), dip(height)),
        move |ui| DatabaseInfoDialog::new(ui, &fields, scanning),
    )
}
