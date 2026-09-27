//! Single-track tag editor (#278): a native modal dialog over the shared
//! [`emusic_ui::tag_editor`] model — the same form, validation and
//! edit/submit/result cycle the dialog drives.
//!
//! The modal's nested loop keeps pumping the main window's messages, so the
//! save round-trip runs through a small [`Bridge`]: the dialog drops its
//! [`EditRequest`] there on Apply (and pokes the main window through a
//! proxy), and the main window mirrors [`TagEditorState::status`] back into
//! it on every tick. The dialog polls the bridge on a timer while a save is
//! in flight.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emusic_ui::library_api::EditRequest;
use emusic_ui::tag_editor::{Failure, Status, TagEditorState, TagForm, TagFormErrors};
use win32ui::prelude::*;
use win32ui::{WindowSpec, dip, row};

use crate::app::Msg as AppMsg;

/// The dialog's client width, in design units.
const WIDTH: f32 = 560.0;
/// The tallest the window gets; past this the form scrolls.
const MAX_HEIGHT: f32 = 640.0;
/// Outer padding of the window layout, in design units.
const MARGIN: f32 = 20.0;
/// Gap between the window layout's blocks, in design units.
const BLOCK_SPACING: f32 = 10.0;
/// Height of the "Edit tags" header line, in design units.
const TITLE_HEIGHT: f32 = 26.0;
/// Height of one field row, in design units.
const ROW_HEIGHT: f32 = 26.0;
/// Height of a numeric field's error row when shown, in design units.
const ERROR_HEIGHT: f32 = 16.0;
/// Gap between the rows of the scrolling panel, in design units.
const ROW_SPACING: f32 = 6.0;
/// Padding inside the scrolling panel, in design units.
const PANEL_MARGIN: f32 = 10.0;
/// Width of the field-name column, in design units.
const LABEL_WIDTH: f32 = 110.0;
/// Height of the status line and of the button row, in design units.
const STATUS_HEIGHT: f32 = 20.0;
const BUTTON_HEIGHT: f32 = 30.0;
/// How often the dialog checks the bridge while a save is in flight.
const POLL_MS: u32 = 100;

/// The field rows, in dialog order; the index doubles as the form slot.
const FIELDS: [&str; 10] = [
    "Title",
    "Artist",
    "Album",
    "Album artist",
    "Genre",
    "Year",
    "Track",
    "Disc",
    "Composer",
    "Comment",
];

/// The fields with a numeric validation rule (Year, Track, Disc).
const NUMERIC: [usize; 3] = [5, 6, 7];

/// The form slot for a field row.
fn field_value(form: &TagForm, index: usize) -> &str {
    match index {
        0 => &form.title,
        1 => &form.artist,
        2 => &form.album,
        3 => &form.album_artist,
        4 => &form.genre,
        5 => &form.year,
        6 => &form.track_no,
        7 => &form.disc_no,
        8 => &form.composer,
        _ => &form.comment,
    }
}

fn set_field(form: &mut TagForm, index: usize, text: String) {
    match index {
        0 => form.title = text,
        1 => form.artist = text,
        2 => form.album = text,
        3 => form.album_artist = text,
        4 => form.genre = text,
        5 => form.year = text,
        6 => form.track_no = text,
        7 => form.disc_no = text,
        8 => form.composer = text,
        _ => form.comment = text,
    }
}

/// The validation error for a field row, if it is one of the numeric fields.
fn field_error(errors: &TagFormErrors, index: usize) -> Option<&str> {
    match index {
        5 => errors.year.as_deref(),
        6 => errors.track_no.as_deref(),
        7 => errors.disc_no.as_deref(),
        _ => None,
    }
}

/// The two-way channel between the modal dialog and the main window: the
/// dialog leaves its save request here; the main window mirrors the shared
/// model's status back.
pub struct Bridge {
    /// The save the dialog asked for, drained by the main window.
    pub apply: Option<EditRequest>,
    /// The editor's status as of the main window's last tick.
    pub status: Status,
}

impl Bridge {
    pub fn new(status: Status) -> Self {
        Self {
            apply: None,
            status,
        }
    }
}

/// The dialog's intents.
pub enum Msg {
    /// A field's text changed (field index, new text).
    Field(usize, String),
    /// Submit the form as a tag edit.
    Apply,
    /// Restore the values the dialog opened with (or last saved).
    Revert,
    /// Close the dialog (Close button or Escape).
    Close,
    /// The save-poll timer fired; check the bridge for an outcome.
    Poll,
}

struct TagEditorDialog {
    form: TagForm,
    original: TagForm,
    path: PathBuf,
    errors: TagFormErrors,
    /// Field indices whose error row is currently expanded in the layout.
    expanded: Vec<usize>,
    bridge: Rc<RefCell<Bridge>>,
    sink: Proxy<AppMsg>,
    pending: bool,
    timer: Option<TimerId>,
    // Held so the controls outlive the layouts that position them.
    _title: FlowText<Msg>,
    _subtitle: FlowText<Msg>,
    labels: Vec<Label>,
    edits: Vec<Edit<Msg>>,
    /// Per numeric field: the indent spring and the error label.
    error_rows: Vec<(usize, Label, Label)>,
    status: Label,
    apply: Button<Msg>,
    revert: Button<Msg>,
    _close: Button<Msg>,
    _spring: Label,
    scroll: ScrollView,
    panel: Panel,
}

impl TagEditorDialog {
    fn new(
        ui: &mut Ui<Msg>,
        state: &TagEditorState,
        bridge: Rc<RefCell<Bridge>>,
        sink: Proxy<AppMsg>,
    ) -> Self {
        let title = FlowText::new(ui)
            .expect("create title")
            .run(Run::normal("Edit tags").weight(600).size(16.0));
        let subtitle = FlowText::new(ui)
            .expect("create subtitle")
            .run(Run::weak(state.path.display().to_string()).size(11.5));

        let scroll = ScrollView::new(ui).expect("create scroll view");
        let panel = Panel::new(ui).expect("create content panel");
        scroll.set_content(&panel);

        let mut panel_ui = panel.ui(ui);
        let mut labels = Vec::new();
        let mut edits = Vec::new();
        let mut error_rows = Vec::new();
        for (index, name) in FIELDS.iter().enumerate() {
            let label = Label::new(&mut panel_ui, Rect::default(), name).expect("create label");
            let edit = Edit::single_line(&mut panel_ui)
                .expect("create field")
                .on_change(move |text| Some(Msg::Field(index, text.to_owned())));
            edit.set_text(field_value(&state.form, index));
            labels.push(label);
            edits.push(edit);
            if NUMERIC.contains(&index) {
                // The error row starts collapsed; a validation failure
                // expands it (see `apply_errors`).
                let spring = Label::new(&mut panel_ui, Rect::default(), "").expect("create spring");
                let error =
                    Label::new(&mut panel_ui, Rect::default(), "").expect("create error label");
                error.set_visible(false);
                error_rows.push((index, spring, error));
            }
        }

        let status = Label::new(ui, Rect::default(), "").expect("create status line");
        let apply = Button::new(ui, "Apply")
            .expect("create apply button")
            .default()
            .on_click(|| Some(Msg::Apply));
        let revert = Button::new(ui, "Revert")
            .expect("create revert button")
            .on_click(|| Some(Msg::Revert));
        let close = Button::new(ui, "Close")
            .expect("create close button")
            .on_click(|| Some(Msg::Close));
        ui.accelerator(Shortcut::key(Key::ESCAPE), || Some(Msg::Close));
        ui.on_timer(|_| Some(Msg::Poll));
        // An empty label springs the buttons to the row's right edge.
        let spring = Label::new(ui, Rect::default(), "").expect("create spring");

        let dialog = Self {
            form: state.form.clone(),
            original: state.original.clone(),
            path: state.path.clone(),
            errors: TagFormErrors::default(),
            expanded: Vec::new(),
            bridge,
            sink,
            pending: false,
            timer: None,
            _title: title,
            _subtitle: subtitle,
            labels,
            edits,
            error_rows,
            status,
            apply,
            revert,
            _close: close,
            _spring: spring,
            scroll,
            panel,
        };
        dialog.relayout(ui);

        let subtitle_height = dialog._subtitle.preferred_height(dip(WIDTH - 2.0 * MARGIN));
        ui.set_layout(
            Layout::column()
                .spacing(dip(BLOCK_SPACING))
                .margins(Insets::all(dip(MARGIN)))
                .item(dialog._title.height(dip(TITLE_HEIGHT)))
                .item(dialog._subtitle.height(subtitle_height))
                .item(dialog.scroll.fill(1))
                .item(dialog.status.height(dip(STATUS_HEIGHT)))
                .item(
                    row![
                        dialog._spring.fill(1),
                        dialog.apply.width(dip(90.0)),
                        dialog.revert.width(dip(90.0)),
                        dialog._close.width(dip(90.0))
                    ]
                    .spacing(dip(8.0))
                    .height(dip(BUTTON_HEIGHT)),
                ),
        );
        dialog
    }

    /// The form's rows as layout items; collapsed error rows are left out
    /// entirely so the row rhythm stays even.
    fn form_rows(&self) -> Vec<(LayoutItem, f32)> {
        let mut rows = Vec::new();
        let mut errors = self.error_rows.iter();
        for (index, (label, edit)) in self.labels.iter().zip(&self.edits).enumerate() {
            rows.push((
                row![label.width(dip(LABEL_WIDTH)), edit.fill(1)]
                    .spacing(dip(8.0))
                    .height(dip(ROW_HEIGHT)),
                ROW_HEIGHT,
            ));
            if NUMERIC.contains(&index) {
                let (_, spring, error) = errors.next().expect("one error row per numeric field");
                if self.expanded.contains(&index) {
                    rows.push((
                        row![spring.width(dip(LABEL_WIDTH)), error.fill(1)]
                            .spacing(dip(8.0))
                            .height(dip(ERROR_HEIGHT)),
                        ERROR_HEIGHT,
                    ));
                }
            }
        }
        rows
    }

    /// (Re)installs the form layout and sizes the scroll extent to it.
    fn relayout(&self, ui: &Ui<Msg>) {
        let (layout, height) = build_form(&self.form_rows());
        self.panel.set_layout(layout);
        self.scroll.set_content_height(height.to_px(ui.dpi()));
        self.panel.relayout();
    }

    /// Pushes the current validation errors into the error rows, expanding or
    /// collapsing them, and enables Apply only when the form is valid.
    fn apply_errors(&mut self, ui: &Ui<Msg>) {
        let expanded: Vec<usize> = NUMERIC
            .into_iter()
            .filter(|index| field_error(&self.errors, *index).is_some())
            .collect();
        for (index, _, label) in &self.error_rows {
            let error = field_error(&self.errors, *index);
            label.set_text(error.unwrap_or(""));
            label.set_visible(error.is_some());
        }
        if expanded != self.expanded {
            self.expanded = expanded;
            self.relayout(ui);
        }
        self.apply
            .set_enabled(!self.pending && self.errors.is_empty());
    }

    fn apply(&mut self, ui: &mut Ui<Msg>) {
        let Ok(tags) = self.form.to_tags() else {
            return;
        };
        self.pending = true;
        self.apply.set_enabled(false);
        self.status.set_text("Applying…");
        self.bridge.borrow_mut().apply = Some(EditRequest::new(self.path.clone(), tags));
        let _ = self.sink.send(AppMsg::TagEditorApply);
        self.timer = ui.set_timer(POLL_MS).ok();
    }

    fn revert(&mut self, ui: &mut Ui<Msg>) {
        self.form = self.original.clone();
        for (index, edit) in self.edits.iter().enumerate() {
            edit.set_text(field_value(&self.form, index));
        }
        self.errors = self.form.validate();
        self.status.set_text("");
        self.apply_errors(ui);
    }

    /// Picks up the save outcome the main window mirrored into the bridge.
    fn poll(&mut self, ui: &mut Ui<Msg>) {
        if !self.pending {
            return;
        }
        let status = self.bridge.borrow().status.clone();
        match status {
            Status::Saved => {
                self.original = self.form.clone();
                self.finish_save(ui, "Saved");
            }
            Status::Failed(failures) => {
                let text = failure_text(&failures);
                self.finish_save(ui, &text);
            }
            _ => {}
        }
    }

    fn finish_save(&mut self, ui: &mut Ui<Msg>, text: &str) {
        self.pending = false;
        if let Some(id) = self.timer.take() {
            ui.kill_timer(id);
        }
        self.status.set_text(text);
        self.apply.set_enabled(self.errors.is_empty());
    }
}

impl App for TagEditorDialog {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Field(index, text) => {
                set_field(&mut self.form, index, text);
                self.errors = self.form.validate();
                self.status.set_text("");
                self.apply_errors(ui);
            }
            Msg::Apply => self.apply(ui),
            Msg::Revert => self.revert(ui),
            Msg::Close => ui.close(),
            Msg::Poll => self.poll(ui),
        }
    }
}

/// The status line for a failed save: the first error, plus a count when
/// more files failed.
fn failure_text(failures: &[Failure]) -> String {
    let Some(first) = failures.first() else {
        return "Save failed".to_owned();
    };
    if failures.len() > 1 {
        format!("{} (+{} more)", first.message, failures.len() - 1)
    } else {
        first.message.clone()
    }
}

/// Builds the panel's form column and the exact content height it needs, so
/// the hosting [`ScrollView`] sizes its scrollbar to the rows.
fn build_form(rows: &[(LayoutItem, f32)]) -> (Layout, Dip) {
    let mut column = Layout::column()
        .spacing(dip(ROW_SPACING))
        .margins(Insets::all(dip(PANEL_MARGIN)));
    let mut height = dip(2.0 * PANEL_MARGIN);
    for (index, (item, row)) in rows.iter().enumerate() {
        if index > 0 {
            height = height + dip(ROW_SPACING);
        }
        height = height + dip(*row);
        column = column.item(item);
    }
    (column, height)
}

/// The window spec: wide enough for comfortable editing, capped so a form
/// with expanded errors scrolls instead of outgrowing the screen.
fn spec() -> WindowSpec {
    const SUBTITLE: f32 = 20.0;
    /// The window frame (`WindowSpec::size` is the outer rect, not the
    /// client area): title bar plus borders.
    const FRAME: f32 = 34.0;
    let rows = FIELDS.len() as f32 * ROW_HEIGHT + (FIELDS.len() - 1) as f32 * ROW_SPACING;
    let content = 2.0 * PANEL_MARGIN + rows;
    let chrome = FRAME
        + 2.0 * MARGIN
        + TITLE_HEIGHT
        + SUBTITLE
        + STATUS_HEIGHT
        + BUTTON_HEIGHT
        + 4.0 * BLOCK_SPACING;
    let height = (chrome + content).min(MAX_HEIGHT);
    WindowSpec::new("Edit tags").size(dip(WIDTH), dip(height))
}

/// Shows the dialog modally over `ui`'s window. `bridge` carries the save
/// request to the main window and the status back; `sink` pokes the main
/// window to drain the bridge.
pub fn show<M: 'static>(
    ui: &Ui<M>,
    state: &TagEditorState,
    bridge: Rc<RefCell<Bridge>>,
    sink: Proxy<AppMsg>,
) {
    ui.open_modal::<TagEditorDialog, _, ()>(spec(), move |ui| {
        TagEditorDialog::new(ui, state, bridge, sink)
    });
}

/// Opens the dialog as a non-modal owned window and returns its handle. The
/// screenshot tool uses this to capture the dialog (a modal's nested loop
/// would block the tool's own capture tick).
pub fn open<M: 'static>(
    ui: &Ui<M>,
    state: &TagEditorState,
    bridge: Rc<RefCell<Bridge>>,
    sink: Proxy<AppMsg>,
) -> win32ui::Result<WindowHandle<Msg>> {
    ui.open_window(spec(), move |ui| {
        TagEditorDialog::new(ui, state, bridge, sink)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_rows_round_trip_the_form() {
        let mut form = TagForm::default();
        for (index, name) in FIELDS.iter().enumerate() {
            set_field(&mut form, index, (*name).to_owned());
        }
        for (index, name) in FIELDS.iter().enumerate() {
            assert_eq!(field_value(&form, index), *name);
        }
    }

    #[test]
    fn only_numeric_fields_have_error_rows() {
        let errors = TagFormErrors {
            year: Some("bad".to_owned()),
            ..TagFormErrors::default()
        };
        assert_eq!(field_error(&errors, 5), Some("bad"));
        assert_eq!(field_error(&errors, 0), None);
        assert_eq!(field_error(&errors, 9), None);
    }

    #[test]
    fn failure_text_shows_the_first_error_and_a_count() {
        let failure = |message: &str| Failure {
            path: String::new(),
            message: message.to_owned(),
        };
        assert_eq!(failure_text(&[failure("denied")]), "denied");
        assert_eq!(
            failure_text(&[failure("denied"), failure("locked")]),
            "denied (+1 more)"
        );
    }
}
