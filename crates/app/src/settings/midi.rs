//! Settings > Playback > "MIDI soundfont": the `.sf2`/`.sf3`/`.sfz` file
//! BASSMIDI renders MIDI files with.
//!
//! BASSMIDI is a software synthesizer, so without a soundfont MIDI files play
//! silently. The path is edited as text or picked with a native file dialog
//! (run on its own thread so the UI never blocks), pushed as
//! [`Command::SetMidiSoundfont`], and checked inline for the common mistakes.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui;
use emusic_player::midi::soundfont_problem;

use crate::state::{AppState, Command};

/// Text-edit buffer and the slot a running file dialog delivers its pick to.
#[derive(Clone, Default)]
struct Editor {
    text: Arc<Mutex<String>>,
    picked: Arc<Mutex<Option<PathBuf>>>,
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let id = egui::Id::new("midi_soundfont_editor");
    let editor: Editor = ui.data_mut(|data| {
        data.get_temp_mut_or_insert_with(id, || Editor {
            text: Arc::new(Mutex::new(path_text(state.midi_soundfont.as_deref()))),
            picked: Arc::default(),
        })
        .clone()
    });

    if let Some(path) = take(&editor.picked) {
        *lock(&editor.text) = path_text(Some(&path));
        state.push(Command::SetMidiSoundfont(Some(path)));
    }

    ui.label("MIDI soundfont");
    ui.horizontal(|ui| {
        let mut text = lock(&editor.text).clone();
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .desired_width(320.0)
                .hint_text("path to a .sf2 / .sf3 / .sfz file"),
        );
        *lock(&editor.text) = text.clone();
        if response.lost_focus() {
            state.push(Command::SetMidiSoundfont(parse(&text)));
        }
        if ui.button("Browse...").clicked() {
            browse(ui.ctx().clone(), editor.picked.clone());
        }
        if ui
            .button("Clear")
            .on_hover_text(
                "Stop using this soundfont. A soundfont already in use stays \
                 active until emusic restarts.",
            )
            .clicked()
        {
            lock(&editor.text).clear();
            state.push(Command::SetMidiSoundfont(None));
        }
    });

    let (message, is_problem) = status(state.midi_soundfont.as_deref());
    let text = egui::RichText::new(message);
    ui.label(if is_problem {
        text.color(ui.visuals().warn_fg_color)
    } else {
        text.weak()
    });
}

/// The inline message under the field and whether it is a warning.
fn status(path: Option<&Path>) -> (String, bool) {
    match path {
        None => (
            "No soundfont set: MIDI files play silently unless a .sf2/.sf3/.sfz \
             file sits next to the BASS DLLs."
                .to_string(),
            true,
        ),
        Some(path) => match soundfont_problem(path) {
            Some(problem) => (
                format!(
                    "{problem} MIDI files play silently unless a soundfont sits \
                     next to the BASS DLLs."
                ),
                true,
            ),
            None => (format!("Using {}", path.display()), false),
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

/// Opens the native file dialog on a background thread and stores the
/// chosen path in `picked`, waking the UI when it's done.
fn browse(ctx: egui::Context, picked: Arc<Mutex<Option<PathBuf>>>) {
    std::thread::spawn(move || {
        let file = rfd::FileDialog::new()
            .add_filter("Soundfont", &["sf2", "sf3", "sfz"])
            .pick_file();
        if let Some(file) = file {
            *lock(&picked) = Some(file);
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
            parse(r#"  "C:\fonts\gm.sf2"  "#),
            Some(PathBuf::from(r"C:\fonts\gm.sf2"))
        );
        assert_eq!(parse("   "), None);
    }

    #[test]
    fn unset_and_missing_soundfonts_warn() {
        let (message, is_problem) = status(None);
        assert!(is_problem && message.contains("silently"));
        let (message, is_problem) = status(Some(Path::new("Z:/definitely/absent.sf2")));
        assert!(is_problem && message.contains("File not found"));
    }
}
