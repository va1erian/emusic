//! File -> Database info (#275, #376): the library counts and database file
//! details from [`emusic_ui::panels::database_info`] in a secondary window,
//! with a "Rescan everything" button.
//!
//! [`show`] runs it as a real modal window on the native Win32 backend and
//! returns the user's choice; on the canvas backend (which cannot run a modal
//! loop on its event-loop thread) it opens the same dialog non-modally instead.
//! [`open`] opens it non-modally for the screenshot tool (a modal's nested loop
//! would block the tool's own capture tick).

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::panels::database_info::fields;
use xui::xui_core::app::{App, Proxy, Ui, WindowHandle};
use xui::xui_core::backend::{PlatformSpec, Result};
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, Label};

use crate::app::Msg as AppMsg;

/// Width of the field-name column, in design units.
const LABEL_WIDTH: f32 = 120.0;
/// Height of one field row, in design units.
const ROW_HEIGHT: f32 = 24.0;
/// Gap between rows, in design units.
const ROW_SPACING: f32 = 4.0;
/// Outer margin of the dialog, in design units.
const MARGIN: f32 = 16.0;
/// Approximate width of one value character, in design units.
const CHAR_WIDTH: f32 = 7.0;
/// Height of the button row, in design units.
const BUTTON_HEIGHT: f32 = 28.0;
/// The dialog's minimum width, in design units.
const MIN_WIDTH: f32 = 400.0;

/// What the user chose in the dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DatabaseInfoChoice {
    /// Rescan every library folder.
    Rescan,
    /// Just close the dialog.
    Close,
}

/// The dialog's intents.
pub enum Msg {
    Rescan,
    Close,
}

/// The Database info dialog: a read-only field list and the Rescan/Close row.
struct DatabaseInfoDialog {
    // Held so the controls outlive their moved rects.
    _labels: Vec<Label<Msg>>,
    _rescan: Button<Msg>,
    _close: Button<Msg>,
    /// Set when the dialog runs non-modally (canvas backend): Rescan reports
    /// through this to the app instead of through a modal result.
    rescan_sink: Option<Proxy<AppMsg>>,
}

impl DatabaseInfoDialog {
    fn new(
        ui: &mut Ui<Msg>,
        fields: &[(&'static str, String)],
        scanning: bool,
        rescan_sink: Option<Proxy<AppMsg>>,
    ) -> Self {
        let client = ui.client_rect();
        let dpi = ui.dpi();
        let margin = dip(MARGIN).to_px(dpi).value();
        let label_w = dip(LABEL_WIDTH).to_px(dpi).value();
        let row_h = dip(ROW_HEIGHT).to_px(dpi).value();
        let spacing = dip(ROW_SPACING).to_px(dpi).value();

        let mut labels = Vec::new();
        let mut moves = Vec::new();
        let mut y = client.top + margin;
        for (name, value) in fields {
            let name = Label::new(ui, Rect::default(), name).expect("create field name label");
            let value = Label::new(ui, Rect::default(), value).expect("create field value label");
            moves.push((
                name.id(),
                Rect::new(
                    client.left + margin,
                    y,
                    client.left + margin + label_w,
                    y + row_h,
                ),
            ));
            moves.push((
                value.id(),
                Rect::new(
                    client.left + margin + label_w + spacing,
                    y,
                    client.right - margin,
                    y + row_h,
                ),
            ));
            labels.push(name);
            labels.push(value);
            y += row_h + spacing;
        }

        let button_h = dip(BUTTON_HEIGHT).to_px(dpi).value();
        let button_y = client.bottom - margin - button_h;
        let close_w = dip(100.0).to_px(dpi).value();
        let rescan_w = dip(170.0).to_px(dpi).value();
        let rescan = Button::new(
            ui,
            Rect::default(),
            if scanning {
                "Scanning..."
            } else {
                "Rescan everything"
            },
        )
        .expect("create rescan button")
        .on_click(|| Some(Msg::Rescan));
        rescan.set_enabled(!scanning);
        let close = Button::new(ui, Rect::default(), "Close")
            .expect("create close button")
            .on_click(|| Some(Msg::Close));
        moves.push((
            rescan.id(),
            Rect::new(
                client.left + margin,
                button_y,
                client.left + margin + rescan_w,
                button_y + button_h,
            ),
        ));
        moves.push((
            close.id(),
            Rect::new(
                client.right - margin - close_w,
                button_y,
                client.right - margin,
                button_y + button_h,
            ),
        ));
        ui.apply_moves(&moves);

        Self {
            _labels: labels,
            _rescan: rescan,
            _close: close,
            rescan_sink,
        }
    }
}

impl App for DatabaseInfoDialog {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        if let Some(sink) = &self.rescan_sink {
            // Non-modal: report the intent to the app and just close.
            if matches!(msg, Msg::Rescan) {
                let _ = sink.send(AppMsg::LibraryRescan);
            }
            ui.close();
        } else {
            ui.close_with_result(match msg {
                Msg::Rescan => DatabaseInfoChoice::Rescan,
                Msg::Close => DatabaseInfoChoice::Close,
            });
        }
    }
}

/// Shows the dialog over `ui`'s window.
///
/// The native Win32 backend runs it as a true modal and returns the user's
/// choice (`None` when it was dismissed with the window's close button). The
/// canvas backend cannot run a modal loop on its event-loop thread, so there it
/// opens the same dialog non-modally: Rescan reports back as
/// [`Msg::LibraryRescan`](crate::app::Msg::LibraryRescan) and this returns
/// `None`.
pub fn show(ui: &Ui<AppMsg>, library: &dyn LibraryDataSource) -> Option<DatabaseInfoChoice> {
    let (spec, fields, scanning) = prepare(library);
    if crate::backend::is_canvas() {
        let rescan_sink = ui.proxy();
        if let Err(error) = ui.open_window(spec, move |ui| {
            DatabaseInfoDialog::new(ui, &fields, scanning, Some(rescan_sink))
        }) {
            tracing::warn!(%error, "could not open the database info window");
        }
        None
    } else {
        ui.open_modal::<DatabaseInfoDialog, _, DatabaseInfoChoice>(spec, move |ui| {
            DatabaseInfoDialog::new(ui, &fields, scanning, None)
        })
    }
}

/// Opens the dialog non-modally and returns its handle, for the screenshot
/// tool.
pub fn open<M: 'static>(ui: &Ui<M>, library: &dyn LibraryDataSource) -> Result<WindowHandle<Msg>> {
    let (spec, fields, scanning) = prepare(library);
    ui.open_window(spec, move |ui| {
        DatabaseInfoDialog::new(ui, &fields, scanning, None)
    })
}

/// Builds the window spec and the field snapshot shared by [`show`]/[`open`].
fn prepare(library: &dyn LibraryDataSource) -> (PlatformSpec, Vec<(&'static str, String)>, bool) {
    let fields = fields(library);
    let scanning = library.is_scanning();
    let longest = fields
        .iter()
        .map(|(_, value)| value.chars().count())
        .max()
        .unwrap_or(0);
    let width = (2.0 * MARGIN + LABEL_WIDTH + longest as f32 * CHAR_WIDTH).max(MIN_WIDTH);
    let height =
        2.0 * MARGIN + fields.len() as f32 * (ROW_HEIGHT + ROW_SPACING) + BUTTON_HEIGHT + 20.0;
    let spec =
        PlatformSpec::new("Database info").size(dip(width), dip(height + super::FRAME_ALLOWANCE));
    (spec, fields, scanning)
}
