//! Track "Properties" dialog (#280): the playing or right-clicked track's
//! full metadata as a native modal window. The sections and value formatting
//! come from [`emusic_ui::views::track_table::properties`], so the dialog
//! shows exactly what the shared model renders.
//!
//! Values are read-only edits: they look like a properties form and stay
//! selectable, so any tag can be copied out. Tall content scrolls inside a
//! fixed-cap window.

use emusic_ui::library_api::TrackInfo;
use emusic_ui::views::track_table::columns;
use emusic_ui::views::track_table::properties::{self, PropertySection};
use xui::prelude::*;
use xui::{WindowSpec, dip, row};

/// The dialog's client width, in design units.
const WIDTH: f32 = 560.0;
/// The tallest the window gets; past this the fields scroll.
const MAX_HEIGHT: f32 = 640.0;
/// Outer padding of the window layout, in design units.
const MARGIN: f32 = 20.0;
/// Gap between the window layout's blocks, in design units.
const BLOCK_SPACING: f32 = 10.0;
/// Height of the title line, in design units.
const TITLE_HEIGHT: f32 = 26.0;
/// Height of the artist/album subtitle, in design units.
const SUBTITLE_HEIGHT: f32 = 20.0;
/// Height of one section header, in design units.
const SECTION_HEIGHT: f32 = 20.0;
/// Height of one field row, in design units.
const ROW_HEIGHT: f32 = 26.0;
/// Gap between the rows of the scrolling panel, in design units.
const ROW_SPACING: f32 = 6.0;
/// Extra breathing room between two sections, in design units.
const SECTION_GAP: f32 = 10.0;
/// Padding inside the scrolling panel, in design units.
const PANEL_MARGIN: f32 = 10.0;
/// Width of the field-name column, in design units.
const LABEL_WIDTH: f32 = 110.0;
/// Height of the Close button row, in design units.
const BUTTON_HEIGHT: f32 = 30.0;

/// The dialog's only intent.
pub enum Msg {
    /// Close the dialog (Close button, Enter, Escape).
    Close,
}

struct PropertiesDialog {
    // Held so the controls outlive the layouts that position them.
    _title: FlowText<Msg>,
    _subtitle: FlowText<Msg>,
    _headers: Vec<FlowText<Msg>>,
    _labels: Vec<Label>,
    _values: Vec<Edit<Msg>>,
    _scroll: ScrollView,
    _panel: Panel,
    _spring: Label,
    _close: Button<Msg>,
}

impl PropertiesDialog {
    fn new(ui: &mut Ui<Msg>, track: &TrackInfo) -> Self {
        let title = FlowText::new(ui).expect("create title").run(
            Run::normal(columns::title_text(track))
                .weight(600)
                .size(16.0),
        );
        let title = title.run(Run::weak(format!("   {}", format_text(track))).size(11.0));

        let mut subtitle_text = columns::artist_text(track).to_owned();
        if !track.album.is_empty() {
            subtitle_text.push_str(" — ");
            subtitle_text.push_str(&track.album);
        }
        let subtitle = FlowText::new(ui)
            .expect("create subtitle")
            .run(Run::weak(subtitle_text).size(11.5));

        let scroll = ScrollView::new(ui).expect("create scroll view");
        let panel = Panel::new(ui).expect("create content panel");
        scroll.set_content(&panel);

        let mut panel_ui = panel.ui(ui);
        let mut headers = Vec::new();
        let mut labels = Vec::new();
        let mut values = Vec::new();
        let mut rows: Vec<(LayoutItem, f32)> = Vec::new();
        for (index, section) in properties::sections(track).iter().enumerate() {
            if index > 0 {
                rows.push((Layout::row().height(dip(SECTION_GAP)), SECTION_GAP));
            }
            headers.push(section_header(&mut panel_ui, section));
            rows.push((
                headers
                    .last()
                    .expect("just pushed")
                    .height(dip(SECTION_HEIGHT)),
                SECTION_HEIGHT,
            ));
            for field in &section.fields {
                let label =
                    Label::new(&mut panel_ui, Rect::default(), field.label).expect("create label");
                let value = Edit::single_line(&mut panel_ui)
                    .expect("create value")
                    .read_only(true);
                value.set_text(&field.value);
                rows.push((
                    row![label.width(dip(LABEL_WIDTH)), value.fill(1)]
                        .spacing(dip(8.0))
                        .height(dip(ROW_HEIGHT)),
                    ROW_HEIGHT,
                ));
                labels.push(label);
                values.push(value);
            }
        }
        let (form, content_height) = build_form(rows);
        panel.set_layout(form);
        scroll.set_content_height(content_height.to_px(ui.dpi()));

        let close = Button::new(ui, "Close")
            .expect("create close button")
            .default()
            .on_click(|| Some(Msg::Close));
        ui.accelerator(Shortcut::key(Key::ESCAPE), || Some(Msg::Close));
        // An empty label springs the Close button to the row's right edge.
        let spring = Label::new(ui, Rect::default(), "").expect("create spring");

        ui.set_layout(
            Layout::column()
                .spacing(dip(BLOCK_SPACING))
                .margins(Insets::all(dip(MARGIN)))
                .item(title.height(dip(TITLE_HEIGHT)))
                .item(subtitle.height(dip(SUBTITLE_HEIGHT)))
                .item(scroll.fill(1))
                .item(row![spring.fill(1), close.width(dip(100.0))].height(dip(BUTTON_HEIGHT))),
        );
        Self {
            _title: title,
            _subtitle: subtitle,
            _headers: headers,
            _labels: labels,
            _values: values,
            _scroll: scroll,
            _panel: panel,
            _spring: spring,
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

/// One weak, small-caps section header.
fn section_header(ui: &mut Ui<Msg>, section: &PropertySection) -> FlowText<Msg> {
    FlowText::new(ui).expect("create section header").run(
        Run::weak(section.title.to_uppercase())
            .weight(600)
            .size(10.5),
    )
}

/// `format (codec)`, e.g. `flac (FLAC)`, for the title line's tail.
fn format_text(track: &TrackInfo) -> String {
    if track.codec.is_empty() {
        track.format.clone()
    } else {
        format!("{} ({})", track.format, track.codec)
    }
}

/// Builds the panel's form column and the exact content height it needs, so
/// the hosting [`ScrollView`] sizes its scrollbar to the rows.
fn build_form(rows: Vec<(LayoutItem, f32)>) -> (Layout, Dip) {
    let mut column = Layout::column()
        .spacing(dip(ROW_SPACING))
        .margins(Insets::all(dip(PANEL_MARGIN)));
    let mut height = dip(2.0 * PANEL_MARGIN);
    for (index, (item, row)) in rows.into_iter().enumerate() {
        if index > 0 {
            height = height + dip(ROW_SPACING);
        }
        height = height + dip(row);
        column = column.item(item);
    }
    (column, height)
}

/// The window's fixed chrome (everything but the scrolling fields), in
/// design units.
fn chrome_height() -> f32 {
    2.0 * MARGIN + TITLE_HEIGHT + SUBTITLE_HEIGHT + BUTTON_HEIGHT + 3.0 * BLOCK_SPACING
}

/// The window spec: wide enough for long paths, capped so tall content
/// scrolls instead of outgrowing the screen.
fn spec(track: &TrackInfo) -> WindowSpec {
    let content = content_height(track);
    let height = (chrome_height() + content).min(MAX_HEIGHT);
    WindowSpec::new("Properties").size(dip(WIDTH), dip(height))
}

/// The height the field sections need, in design units (kept in sync with
/// the panel layout above).
fn content_height(track: &TrackInfo) -> f32 {
    let sections = properties::sections(track);
    let rows: usize = sections.iter().map(|section| section.fields.len()).sum();
    let items = sections.len() + rows + sections.len().saturating_sub(1);
    2.0 * PANEL_MARGIN
        + sections.len() as f32 * SECTION_HEIGHT
        + rows as f32 * ROW_HEIGHT
        + sections.len().saturating_sub(1) as f32 * SECTION_GAP
        + items.saturating_sub(1) as f32 * ROW_SPACING
}

/// Shows the dialog modally over `ui`'s window.
pub fn show<M: 'static>(ui: &Ui<M>, track: &TrackInfo) {
    let spec = spec(track);
    let track = track.clone();
    ui.open_modal::<PropertiesDialog, _, ()>(spec, move |ui| PropertiesDialog::new(ui, &track));
}

/// Opens the dialog as a non-modal owned window and returns its handle. The
/// screenshot tool uses this to capture the dialog (a modal's nested loop
/// would block the tool's own capture tick).
pub fn open<M: 'static>(ui: &Ui<M>, track: &TrackInfo) -> xui::Result<WindowHandle<Msg>> {
    let spec = spec(track);
    let track = track.clone();
    ui.open_window(spec, move |ui| PropertiesDialog::new(ui, &track))
}
