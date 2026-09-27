//! Single-track tag editor (#278, #376): a secondary window over the shared
//! [`emusic_ui::tag_editor`] model — the same form, validation and
//! edit/submit/result cycle the dialog drives.
//!
//! The editor runs as an *owned, non-modal* window (a modal's nested loop
//! defers the opener's `update`, so the save round-trip could not run). It
//! talks to the main window through a small [`Bridge`]: it drops its
//! [`EditRequest`] there on Apply and pokes the opener through a [`Proxy`], and
//! the opener mirrors [`TagEditorState::status`] back into the bridge on every
//! tick. The dialog polls the bridge on a timer while a save is in flight.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emusic_ui::library_api::EditRequest;
use emusic_ui::tag_editor::{Failure, Status, TagEditorState, TagForm, TagFormErrors};
use xui::xui_core::app::{App, Proxy, Ui, WindowHandle};
use xui::xui_core::backend::{PlatformSpec, Result, TimerId};
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, Edit, HasText, Label};

use crate::app::Msg as AppMsg;

/// The dialog's client width, in design units.
const WIDTH: f32 = 560.0;
/// Outer margin of the dialog, in design units.
const MARGIN: f32 = 20.0;
/// Gap between the dialog's blocks, in design units.
const BLOCK_SPACING: f32 = 8.0;
/// Height of the "Edit tags" header, in design units.
const TITLE_HEIGHT: f32 = 26.0;
/// Height of the file path subtitle, in design units.
const SUBTITLE_HEIGHT: f32 = 20.0;
/// Height of one field row, in design units.
const ROW_HEIGHT: f32 = 26.0;
/// Gap between field rows, in design units.
const ROW_SPACING: f32 = 4.0;
/// Height of a numeric field's error row when shown, in design units.
const ERROR_HEIGHT: f32 = 16.0;
/// Height of the status line and of the button row, in design units.
const STATUS_HEIGHT: f32 = 20.0;
const BUTTON_HEIGHT: f32 = 30.0;
/// Width of the field-name column, in design units.
const LABEL_WIDTH: f32 = 110.0;
/// Gap between a label and its field, in design units.
const LABEL_GAP: f32 = 8.0;
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

/// The two-way channel between the editor window and the main window: the
/// dialog leaves its save request here; the main window mirrors the shared
/// model's status back.
pub struct Bridge {
    /// The save the dialog asked for, drained by the main window.
    pub apply: Option<EditRequest>,
    /// The editor's status as of the main window's last tick.
    pub status: Status,
}

impl Bridge {
    /// A bridge seeded with the editor's current status.
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

/// The tag editor window.
struct TagEditorDialog {
    form: TagForm,
    original: TagForm,
    path: PathBuf,
    errors: TagFormErrors,
    bridge: Rc<RefCell<Bridge>>,
    sink: Proxy<AppMsg>,
    pending: bool,
    timer: Option<TimerId>,
    // Held so the controls outlive their moved rects.
    _title: Label<Msg>,
    _subtitle: Label<Msg>,
    _labels: Vec<Label<Msg>>,
    edits: Vec<Edit<Msg>>,
    error_labels: Vec<(usize, Label<Msg>)>,
    status: Label<Msg>,
    apply: Button<Msg>,
    _revert: Button<Msg>,
    _close: Button<Msg>,
}

impl TagEditorDialog {
    fn new(
        ui: &mut Ui<Msg>,
        state: &TagEditorState,
        bridge: Rc<RefCell<Bridge>>,
        sink: Proxy<AppMsg>,
    ) -> Self {
        let client = ui.client_rect();
        let dpi = ui.dpi();
        let margin = dip(MARGIN).to_px(dpi).value();
        let label_w = dip(LABEL_WIDTH).to_px(dpi).value();
        let label_gap = dip(LABEL_GAP).to_px(dpi).value();
        let row_h = dip(ROW_HEIGHT).to_px(dpi).value();
        let spacing = dip(ROW_SPACING).to_px(dpi).value();
        let error_h = dip(ERROR_HEIGHT).to_px(dpi).value();
        let block = dip(BLOCK_SPACING).to_px(dpi).value();
        let field_left = client.left + margin + label_w + label_gap;

        let title = Label::new(ui, Rect::default(), "Edit tags").expect("create title");
        let subtitle = Label::new(ui, Rect::default(), &state.path.display().to_string())
            .expect("create subtitle");

        let mut labels = Vec::new();
        let mut edits = Vec::new();
        let mut error_labels = Vec::new();
        let mut moves = Vec::new();
        let mut y = client.top + margin;
        moves.push((
            title.id(),
            Rect::new(
                client.left + margin,
                y,
                client.right - margin,
                y + dip(TITLE_HEIGHT).to_px(dpi).value(),
            ),
        ));
        y += dip(TITLE_HEIGHT).to_px(dpi).value() + block;
        moves.push((
            subtitle.id(),
            Rect::new(
                client.left + margin,
                y,
                client.right - margin,
                y + dip(SUBTITLE_HEIGHT).to_px(dpi).value(),
            ),
        ));
        y += dip(SUBTITLE_HEIGHT).to_px(dpi).value() + block;

        for (index, name) in FIELDS.iter().enumerate() {
            let label = Label::new(ui, Rect::default(), name).expect("create label");
            let edit = Edit::new(ui, Rect::default(), field_value(&state.form, index))
                .expect("create field")
                .on_change(move |text| Some(Msg::Field(index, text.to_owned())));
            moves.push((
                label.id(),
                Rect::new(
                    client.left + margin,
                    y,
                    client.left + margin + label_w,
                    y + row_h,
                ),
            ));
            moves.push((
                edit.id(),
                Rect::new(field_left, y, client.right - margin, y + row_h),
            ));
            labels.push(label);
            edits.push(edit);
            y += row_h + spacing;
            if NUMERIC.contains(&index) {
                let error = Label::new(ui, Rect::default(), "").expect("create error label");
                moves.push((
                    error.id(),
                    Rect::new(field_left, y, client.right - margin, y + error_h),
                ));
                error_labels.push((index, error));
                y += error_h;
            }
        }

        let status = Label::new(ui, Rect::default(), "").expect("create status line");
        let apply = Button::new(ui, Rect::default(), "Apply")
            .expect("create apply button")
            .on_click(|| Some(Msg::Apply));
        let revert = Button::new(ui, Rect::default(), "Revert")
            .expect("create revert button")
            .on_click(|| Some(Msg::Revert));
        let close = Button::new(ui, Rect::default(), "Close")
            .expect("create close button")
            .on_click(|| Some(Msg::Close));

        let status_h = dip(STATUS_HEIGHT).to_px(dpi).value();
        let button_h = dip(BUTTON_HEIGHT).to_px(dpi).value();
        let button_w = dip(90.0).to_px(dpi).value();
        let bottom = client.bottom - margin;
        let button_y = bottom - button_h;
        let status_y = button_y - block - status_h;
        moves.push((
            status.id(),
            Rect::new(
                client.left + margin,
                status_y,
                client.right - margin,
                status_y + status_h,
            ),
        ));
        let mut x = client.right - margin;
        for button in [&close, &revert, &apply] {
            moves.push((button.id(), Rect::new(x - button_w, button_y, x, bottom)));
            x -= button_w + dip(8.0).to_px(dpi).value();
        }
        ui.apply_moves(&moves);

        let errors = state.form.validate();
        let mut dialog = Self {
            form: state.form.clone(),
            original: state.original.clone(),
            path: state.path.clone(),
            errors,
            bridge,
            sink,
            pending: false,
            timer: None,
            _title: title,
            _subtitle: subtitle,
            _labels: labels,
            edits,
            error_labels,
            status,
            apply,
            _revert: revert,
            _close: close,
        };
        dialog.refresh_errors();
        dialog
    }

    /// Pushes the validation errors into the error labels and enables Apply
    /// only when the form is valid and no save is pending.
    fn refresh_errors(&mut self) {
        for (index, label) in &self.error_labels {
            let error = field_error(&self.errors, *index);
            label.set_text(error.unwrap_or(""));
        }
        let enabled = !self.pending && self.errors.is_empty();
        self.apply.set_enabled(enabled);
    }

    fn apply(&mut self, ui: &mut Ui<Msg>) {
        let Ok(tags) = self.form.to_tags() else {
            return;
        };
        self.pending = true;
        self.apply.set_enabled(false);
        self.status.set_text("Applying...");
        self.bridge.borrow_mut().apply = Some(EditRequest::new(self.path.clone(), tags));
        let _ = self.sink.send(AppMsg::TagEditorApply);
        let timer = ui.set_timer(POLL_MS);
        if timer.0 != 0 {
            self.timer = Some(timer);
        }
    }

    fn revert(&mut self) {
        self.form = self.original.clone();
        for (index, edit) in self.edits.iter().enumerate() {
            edit.set_text(field_value(&self.form, index));
        }
        self.errors = self.form.validate();
        self.status.set_text("");
        self.refresh_errors();
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
                self.refresh_errors();
            }
            Msg::Apply => self.apply(ui),
            Msg::Revert => self.revert(),
            Msg::Close => ui.close(),
            Msg::Poll => self.poll(ui),
        }
    }
}

/// The status line for a failed save: the first error, plus a count when more
/// files failed.
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

/// Opens the editor as a non-modal owned window and returns its handle.
///
/// `bridge` carries the save request to the main window and the status back;
/// `sink` pokes the main window to drain the bridge.
pub fn open<M: 'static>(
    ui: &Ui<M>,
    state: &TagEditorState,
    bridge: Rc<RefCell<Bridge>>,
    sink: Proxy<AppMsg>,
) -> Result<WindowHandle<Msg>> {
    let spec = spec();
    let state = clone_state(state);
    ui.open_window(spec, move |ui| {
        TagEditorDialog::new(ui, &state, bridge, sink)
    })
}

/// A clone of the editor's opening state, so the window closure owns it.
fn clone_state(state: &TagEditorState) -> TagEditorState {
    TagEditorState {
        path: state.path.clone(),
        original: state.original.clone(),
        form: state.form.clone(),
        status: state.status.clone(),
        auto_tag: emusic_ui::tag_editor::AutoTagState::Idle,
    }
}

/// The window spec, fitted to the form (kept in sync with the layout above).
fn spec() -> PlatformSpec {
    let rows = FIELDS.len() as f32 * (ROW_HEIGHT + ROW_SPACING);
    let errors = NUMERIC.len() as f32 * ERROR_HEIGHT;
    let height = 2.0 * MARGIN
        + TITLE_HEIGHT
        + SUBTITLE_HEIGHT
        + 2.0 * BLOCK_SPACING
        + rows
        + errors
        + BLOCK_SPACING
        + STATUS_HEIGHT
        + BLOCK_SPACING
        + BUTTON_HEIGHT
        + 10.0;
    PlatformSpec::new("Edit tags").size(dip(WIDTH), dip(height + super::FRAME_ALLOWANCE))
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
