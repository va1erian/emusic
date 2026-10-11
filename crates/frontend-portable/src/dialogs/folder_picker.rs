//! Settings -> Library -> Add folder, on a host with no platform folder dialog
//! (LazyOS has none): a small window that browses the folders under a start
//! folder and hands the chosen one to the shell as a `LibraryAddFolder`
//! command, which records it in the state and the config and tells the library.
//!
//! The window lists the current folder's subfolders (`..` first). Activating a
//! row (double-click, Enter) enters it; **Choose** takes the selected folder, or
//! the current one when none is selected.

use std::path::{Path, PathBuf};

use emusic_ui::state::Command;
use xui::xui_core::app::{App, Proxy, Ui, WindowHandle};
use xui::xui_core::arrange::{button, label, list};
use xui::xui_core::backend::PlatformSpec;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, HasText, Label, ListView};

use crate::app::Msg as AppMsg;
use crate::make::make;

/// The window's client size, in design units.
const WIDTH: f32 = 480.0;
const HEIGHT: f32 = 400.0;
/// Outer margin, in design units.
const MARGIN: f32 = 16.0;
/// Height of a text line and of the button row, in design units.
const ROW_HEIGHT: f32 = 26.0;
/// Width of a button, in design units.
const BUTTON_WIDTH: f32 = 110.0;
/// Gap between controls, in design units.
const GAP: f32 = 8.0;

/// The dialog's intents.
pub enum Msg {
    /// A row was selected.
    Select,
    /// A row was activated: enter that folder (or go up for `..`).
    Activate(usize),
    /// Take the selected folder, or the current one.
    Choose,
    /// Close without choosing.
    Cancel,
}

/// The subfolder names of `dir`, sorted case-insensitively, hidden ones last
/// out of the way (dot-prefixed names are left out).
fn subfolders(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names
}

/// The folder picker window.
struct FolderPicker {
    dir: PathBuf,
    /// The rows after the leading `..`.
    names: Vec<String>,
    list: ListView<Msg>,
    path: Label<Msg>,
    // Held so the controls outlive their moved rects.
    _title: Label<Msg>,
    _choose: Button<Msg>,
    _cancel: Button<Msg>,
    sink: Proxy<AppMsg>,
}

impl FolderPicker {
    fn new(ui: &mut Ui<Msg>, start: PathBuf, sink: Proxy<AppMsg>) -> Self {
        let client = ui.client_rect();
        let dpi = ui.dpi();
        let px = |value: f32| dip(value).to_px(dpi).value();
        let (margin, row, gap, button_w) = (px(MARGIN), px(ROW_HEIGHT), px(GAP), px(BUTTON_WIDTH));

        let title = make(ui, label("Choose a folder with music")).expect("create title");
        let path = make(ui, label("")).expect("create path label");
        let list = make(ui, list())
            .expect("create folder list")
            .on_select(|_| Some(Msg::Select))
            .on_activate(|index| Some(Msg::Activate(index)));
        let choose = make(ui, button("Choose"))
            .expect("create choose button")
            .on_click(|| Some(Msg::Choose));
        let cancel = make(ui, button("Cancel"))
            .expect("create cancel button")
            .on_click(|| Some(Msg::Cancel));

        let left = client.left + margin;
        let right = client.right - margin;
        let buttons_top = client.bottom - margin - row;
        let list_top = client.top + margin + 2 * row + gap;
        ui.apply_moves(&[
            (
                title.id(),
                Rect::new(left, client.top + margin, right, client.top + margin + row),
            ),
            (
                path.id(),
                Rect::new(
                    left,
                    client.top + margin + row,
                    right,
                    client.top + margin + 2 * row,
                ),
            ),
            (
                list.id(),
                Rect::new(left, list_top, right, buttons_top - gap),
            ),
            (
                choose.id(),
                Rect::new(
                    right - 2 * button_w - gap,
                    buttons_top,
                    right - button_w - gap,
                    buttons_top + row,
                ),
            ),
            (
                cancel.id(),
                Rect::new(right - button_w, buttons_top, right, buttons_top + row),
            ),
        ]);

        let mut picker = Self {
            dir: start,
            names: Vec::new(),
            list,
            path,
            _title: title,
            _choose: choose,
            _cancel: cancel,
            sink,
        };
        picker.show_dir();
        picker
    }

    /// Lists the current folder and shows its path.
    fn show_dir(&mut self) {
        self.names = subfolders(&self.dir);
        let mut rows = vec![".."];
        rows.extend(self.names.iter().map(String::as_str));
        self.list.set_items(&rows);
        self.list.select(None);
        self.path.set_text(&self.dir.display().to_string());
    }

    /// The folder `Choose` would take: the selected row, or the current one.
    fn candidate(&self) -> PathBuf {
        match self.list.selected() {
            Some(row) if row > 0 => self
                .names
                .get(row - 1)
                .map_or_else(|| self.dir.clone(), |name| self.dir.join(name)),
            _ => self.dir.clone(),
        }
    }
}

impl App for FolderPicker {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select => {
                self.path.set_text(&self.candidate().display().to_string());
            }
            Msg::Activate(0) => {
                if let Some(parent) = self.dir.parent().filter(|p| !p.as_os_str().is_empty()) {
                    self.dir = parent.to_path_buf();
                    self.show_dir();
                }
            }
            Msg::Activate(row) => {
                if let Some(name) = self.names.get(row - 1) {
                    self.dir = self.dir.join(name);
                    self.show_dir();
                }
            }
            Msg::Choose => {
                // Through the shell like the Remove button: it records the
                // folder in the state and the config, then tells the library.
                let _ = self.sink.send(AppMsg::Dispatch(Command::LibraryAddFolder(
                    self.candidate(),
                )));
                ui.close();
            }
            Msg::Cancel => ui.close(),
        }
    }
}

/// The folder the picker starts in: `~/Music` when it exists, else the home
/// folder, else the root.
#[must_use]
pub fn start_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(home) = home {
        let music = home.join("Music");
        if music.is_dir() {
            return music;
        }
        if home.is_dir() {
            return home;
        }
    }
    PathBuf::from("/")
}

/// Opens the picker over `ui`'s window, starting in `start`; `None` when the
/// window could not be opened.
pub fn show(ui: &Ui<AppMsg>, start: PathBuf) -> Option<WindowHandle<Msg>> {
    let sink = ui.proxy();
    let spec = PlatformSpec::new("Add music folder")
        .size(dip(WIDTH), dip(HEIGHT + super::FRAME_ALLOWANCE));
    match ui.open_window(spec, move |ui| FolderPicker::new(ui, start, sink)) {
        Ok(window) => Some(window),
        Err(error) => {
            tracing::warn!(%error, "could not open the folder picker window");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subfolders_are_sorted_and_skip_files_and_hidden_ones() {
        let root = std::env::temp_dir().join(format!("emusic-folders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for name in ["beta", "Alpha", ".hidden"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        std::fs::write(root.join("song.mp3"), b"x").unwrap();

        assert_eq!(subfolders(&root), ["Alpha", "beta"]);
        assert!(subfolders(&root.join("missing")).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
