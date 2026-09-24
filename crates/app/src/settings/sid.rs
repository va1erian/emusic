//! Settings > Playback > "SID song lengths" (#192): where the HVSC
//! `Songlengths.md5` database lives, and the fallback play length for tunes
//! it doesn't cover.
//!
//! SID files carry no length, so without a database the player can't show a
//! real total (it shows elapsed time only and stops after the fallback
//! length, so the queue still advances). The path may be either the
//! `Songlengths.md5` file or an HVSC root folder, in which the database is
//! auto-detected at `DOCUMENTS/Songlengths.md5`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui;
use emusic_player::sid::resolve_database_path;

use crate::state::{AppState, Command};

/// Text-edit buffer and the slot a running file/folder dialog delivers its
/// pick to.
#[derive(Clone, Default)]
struct Editor {
    text: Arc<Mutex<String>>,
    picked: Arc<Mutex<Option<PathBuf>>>,
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let id = egui::Id::new("songlengths_editor");
    let editor: Editor = ui.data_mut(|data| {
        data.get_temp_mut_or_insert_with(id, || Editor {
            text: Arc::new(Mutex::new(path_text(state.songlengths_path.as_deref()))),
            picked: Arc::default(),
        })
        .clone()
    });

    if let Some(path) = take(&editor.picked) {
        *lock(&editor.text) = path_text(Some(&path));
        state.push(Command::SetSonglengthsPath(Some(path)));
    }

    ui.label("HVSC Songlengths database");
    ui.horizontal(|ui| {
        let mut text = lock(&editor.text).clone();
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .desired_width(320.0)
                .hint_text("Songlengths.md5, or an HVSC root folder"),
        );
        *lock(&editor.text) = text.clone();
        if response.lost_focus() {
            state.push(Command::SetSonglengthsPath(parse(&text)));
        }
        if ui.button("Browse file...").clicked() {
            browse(ui.ctx().clone(), editor.picked.clone(), false);
        }
        if ui.button("Browse folder...").clicked() {
            browse(ui.ctx().clone(), editor.picked.clone(), true);
        }
        if ui.button("Clear").clicked() {
            lock(&editor.text).clear();
            state.push(Command::SetSonglengthsPath(None));
        }
    });

    let (message, is_problem) = status(state.songlengths_path.as_deref());
    let text = egui::RichText::new(message);
    ui.label(if is_problem {
        text.color(ui.visuals().warn_fg_color)
    } else {
        text.weak()
    });

    ui.add_space(4.0);
    let mut fallback = state.sid_fallback_secs;
    if ui
        .add(
            egui::Slider::new(&mut fallback, 30..=600)
                .suffix(" s")
                .text("fallback length"),
        )
        .changed()
    {
        state.push(Command::SetSidFallbackSecs(fallback));
    }
    ui.label(
        egui::RichText::new(
            "SID tunes with no database entry play for this long, without a \
             shown total, so the queue still advances.",
        )
        .weak(),
    );
}

/// The inline message under the field and whether it is a warning.
fn status(path: Option<&Path>) -> (String, bool) {
    match path {
        None => (
            "No database: SID tunes show elapsed time only and stop after the \
             fallback length."
                .to_string(),
            true,
        ),
        Some(path) => match resolve_database_path(path) {
            Some(database) => (format!("Using {}", database.display()), false),
            None => (
                format!("No Songlengths.md5 found at {}", path.display()),
                true,
            ),
        },
    }
}

fn path_text(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_default()
}

fn parse(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim().trim_matches('"');
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn take(slot: &Mutex<Option<PathBuf>>) -> Option<PathBuf> {
    lock(slot).take()
}

/// Opens the native file/folder dialog on a background thread and stores the
/// chosen path in `picked`, waking the UI when it's done.
fn browse(ctx: egui::Context, picked: Arc<Mutex<Option<PathBuf>>>, folder: bool) {
    std::thread::spawn(move || {
        let dialog = rfd::FileDialog::new().add_filter("Songlengths", &["md5", "txt"]);
        let choice = if folder {
            dialog.pick_folder()
        } else {
            dialog.pick_file()
        };
        if let Some(path) = choice {
            *lock(&picked) = Some(path);
            ctx.request_repaint();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trims_whitespace_and_quotes() {
        assert_eq!(
            parse(r#"  "C:\hvsc\DOCUMENTS\Songlengths.md5"  "#),
            Some(PathBuf::from(r"C:\hvsc\DOCUMENTS\Songlengths.md5"))
        );
        assert_eq!(parse("   "), None);
    }

    #[test]
    fn unset_and_missing_paths_warn() {
        let (message, is_problem) = status(None);
        assert!(is_problem && message.contains("elapsed time only"));
        let (message, is_problem) = status(Some(Path::new("Z:/definitely/absent")));
        assert!(is_problem && message.contains("No Songlengths.md5"));
    }
}
