//! Track "Properties" dialog (#280, #376): the playing or right-clicked
//! track's full metadata in a secondary read-only window. The sections and
//! value formatting come from [`emusic_ui::views::track_table::properties`],
//! so the dialog shows exactly what the shared model renders.
//!
//! Values are drawn as labels: this portable build has no read-only `Edit`, so
//! they are display-only (selectable text is a follow-up once xui exposes a
//! read-only editor).

use emusic_ui::library_api::TrackInfo;
use emusic_ui::views::track_table::columns;
use emusic_ui::views::track_table::properties::{self};
use xui::xui_core::app::{App, Ui, WindowHandle};
use xui::xui_core::backend::{PlatformSpec, Result};
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, Label};

/// The dialog's client width, in design units.
const WIDTH: f32 = 560.0;
/// Outer margin of the dialog, in design units.
const MARGIN: f32 = 20.0;
/// Height of the title line, in design units.
const TITLE_HEIGHT: f32 = 26.0;
/// Height of the artist/album subtitle, in design units.
const SUBTITLE_HEIGHT: f32 = 20.0;
/// Height of one section header, in design units.
const SECTION_HEIGHT: f32 = 18.0;
/// Height of one field row, in design units.
const ROW_HEIGHT: f32 = 22.0;
/// Gap between rows, in design units.
const ROW_SPACING: f32 = 2.0;
/// Extra gap between two sections, in design units.
const SECTION_GAP: f32 = 8.0;
/// Height of the Close button row, in design units.
const BUTTON_HEIGHT: f32 = 30.0;
/// Width of the field-name column, in design units.
const LABEL_WIDTH: f32 = 120.0;
/// Gap between the label and the value, in design units.
const LABEL_GAP: f32 = 8.0;

/// The dialog's only intent.
pub enum Msg {
    /// Close the dialog (Close button, Enter, Escape).
    Close,
}

/// The Properties dialog.
struct PropertiesDialog {
    // Held so the controls outlive their moved rects.
    _labels: Vec<Label<Msg>>,
    _close: Button<Msg>,
}

impl PropertiesDialog {
    fn new(ui: &mut Ui<Msg>, track: &TrackInfo) -> Self {
        let client = ui.client_rect();
        let dpi = ui.dpi();
        let margin = dip(MARGIN).to_px(dpi).value();
        let label_w = dip(LABEL_WIDTH).to_px(dpi).value();
        let label_gap = dip(LABEL_GAP).to_px(dpi).value();
        let value_left = client.left + margin + label_w + label_gap;
        let value_right = client.right - margin;

        let mut labels = Vec::new();
        let mut moves = Vec::new();
        let mut y = client.top + margin;

        let title = Label::new(ui, Rect::default(), columns::title_text(track))
            .expect("create title label");
        moves.push((
            title.id(),
            Rect::new(
                client.left + margin,
                y,
                value_right,
                y + dip(TITLE_HEIGHT).to_px(dpi).value(),
            ),
        ));
        labels.push(title);
        y += dip(TITLE_HEIGHT).to_px(dpi).value();

        let mut subtitle_text = columns::artist_text(track).to_owned();
        if !track.album.is_empty() {
            subtitle_text.push_str(" \u{2014} ");
            subtitle_text.push_str(&track.album);
        }
        let subtitle =
            Label::new(ui, Rect::default(), &subtitle_text).expect("create subtitle label");
        moves.push((
            subtitle.id(),
            Rect::new(
                client.left + margin,
                y,
                value_right,
                y + dip(SUBTITLE_HEIGHT).to_px(dpi).value(),
            ),
        ));
        labels.push(subtitle);
        y += dip(SUBTITLE_HEIGHT).to_px(dpi).value();

        let row_h = dip(ROW_HEIGHT).to_px(dpi).value();
        let section_h = dip(SECTION_HEIGHT).to_px(dpi).value();
        let spacing = dip(ROW_SPACING).to_px(dpi).value();
        let section_gap = dip(SECTION_GAP).to_px(dpi).value();

        for (index, section) in properties::sections(track).iter().enumerate() {
            if index > 0 {
                y += section_gap;
            }
            let header = Label::new(ui, Rect::default(), &section.title.to_uppercase())
                .expect("create header");
            moves.push((
                header.id(),
                Rect::new(client.left + margin, y, value_right, y + section_h),
            ));
            labels.push(header);
            y += section_h + spacing;
            for field in &section.fields {
                let label = Label::new(ui, Rect::default(), field.label).expect("create label");
                let value = Label::new(ui, Rect::default(), &field.value).expect("create value");
                moves.push((
                    label.id(),
                    Rect::new(
                        client.left + margin,
                        y,
                        client.left + margin + label_w,
                        y + row_h,
                    ),
                ));
                moves.push((value.id(), Rect::new(value_left, y, value_right, y + row_h)));
                labels.push(label);
                labels.push(value);
                y += row_h + spacing;
            }
        }

        let button_h = dip(BUTTON_HEIGHT).to_px(dpi).value();
        let button_w = dip(100.0).to_px(dpi).value();
        let close = Button::new(ui, Rect::default(), "Close")
            .expect("create close button")
            .on_click(|| Some(Msg::Close));
        moves.push((
            close.id(),
            Rect::new(
                client.right - margin - button_w,
                client.bottom - margin - button_h,
                client.right - margin,
                client.bottom - margin,
            ),
        ));
        ui.apply_moves(&moves);

        Self {
            _labels: labels,
            _close: close,
        }
    }
}

impl App for PropertiesDialog {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Close => ui.close(),
        }
    }
}

/// Shows the dialog modally over `ui`'s window.
pub fn show<M: 'static>(ui: &Ui<M>, track: &TrackInfo) {
    let spec = spec(track);
    let track = track.clone();
    ui.open_modal::<PropertiesDialog, _, ()>(spec, move |ui| PropertiesDialog::new(ui, &track));
}

/// Opens the dialog as a non-modal owned window and returns its handle, for the
/// screenshot tool.
pub fn open<M: 'static>(ui: &Ui<M>, track: &TrackInfo) -> Result<WindowHandle<Msg>> {
    let spec = spec(track);
    let track = track.clone();
    ui.open_window(spec, move |ui| PropertiesDialog::new(ui, &track))
}

/// The window spec: fitted to the section rows (kept in sync with the layout
/// above), so no scrolling is needed.
fn spec(track: &TrackInfo) -> PlatformSpec {
    let sections = properties::sections(track);
    let rows: usize = sections.iter().map(|section| section.fields.len()).sum();
    let content = 2.0 * MARGIN
        + TITLE_HEIGHT
        + SUBTITLE_HEIGHT
        + sections.len() as f32 * SECTION_HEIGHT
        + rows as f32 * ROW_HEIGHT
        + sections.len().saturating_sub(1) as f32 * SECTION_GAP
        + (sections.len() + rows).saturating_sub(1) as f32 * ROW_SPACING
        + BUTTON_HEIGHT
        + 10.0;
    PlatformSpec::new("Properties").size(dip(WIDTH), dip(content + super::FRAME_ALLOWANCE))
}
