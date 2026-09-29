//! A small modal name prompt (#476): one text field and OK / Cancel.
//!
//! Playlist creation and renaming ask for a name through this dialog, which
//! mirrors [`tag_editor`](super::tag_editor)'s modal shape but with a single
//! value. It answers with the trimmed name, or `None` when cancelled.

use win32ui::prelude::*;
use win32ui::{WindowSpec, dip, row};

/// The dialog's client width, in design units.
const WIDTH: f32 = 380.0;
/// Height of the text field and of the button row, in design units.
const ROW_HEIGHT: f32 = 28.0;
/// Outer margin, in design units.
const MARGIN: f32 = 16.0;
/// Gap between the field and the buttons, in design units.
const SPACING: f32 = 12.0;
/// Total client height, in design units.
const HEIGHT: f32 = MARGIN * 2.0 + ROW_HEIGHT * 2.0 + SPACING + 24.0;

enum Msg {
    Submit,
    Cancel,
}

struct NamePrompt {
    edit: Edit<Msg>,
    _ok: Button<Msg>,
    _cancel: Button<Msg>,
}

impl NamePrompt {
    fn new(ui: &mut Ui<Msg>, initial: &str) -> Self {
        let edit = Edit::single_line(ui).expect("create name field");
        edit.set_text(initial);
        edit.select_all();
        edit.focus();
        let ok = Button::new(ui, "OK")
            .expect("create OK button")
            .default()
            .on_click(|| Some(Msg::Submit));
        let cancel = Button::new(ui, "Cancel")
            .expect("create cancel button")
            .on_click(|| Some(Msg::Cancel));
        ui.accelerator(Shortcut::key(Key::ESCAPE), || Some(Msg::Cancel));
        let spring = Label::new(ui, Rect::default(), "").expect("create spring");
        ui.set_layout(
            Layout::column()
                .spacing(dip(SPACING))
                .margins(Insets::all(dip(MARGIN)))
                .item(edit.height(dip(ROW_HEIGHT)))
                .item(
                    row![spring.fill(1), ok.width(dip(90.0)), cancel.width(dip(90.0))]
                        .spacing(dip(8.0))
                        .height(dip(ROW_HEIGHT)),
                ),
        );
        Self {
            edit,
            _ok: ok,
            _cancel: cancel,
        }
    }
}

impl App for NamePrompt {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Submit => ui.close_with_result(self.edit.text().trim().to_owned()),
            Msg::Cancel => ui.close(),
        }
    }
}

/// Shows the prompt modally, returning the trimmed name or `None` when
/// cancelled. An empty name is returned as an empty string; the caller decides
/// what that means.
pub fn show<M: 'static>(ui: &Ui<M>, title: &str, initial: &str) -> Option<String> {
    let initial = initial.to_owned();
    ui.open_modal::<NamePrompt, _, String>(
        WindowSpec::new(title).size(dip(WIDTH), dip(HEIGHT)),
        move |ui| NamePrompt::new(ui, &initial),
    )
}
