//! "File associations" settings page (#11): a checkbox per extension emusic
//! can play, `Register`/`Unregister all` buttons, and a shortcut to
//! Windows' own Default apps settings.
//!
//! Windows 10/11 does not let an app make itself the default handler
//! programmatically (the per-type `UserChoice` is hash-protected and guarded
//! by `UCPD.sys`). Registering therefore only makes emusic *available*; after
//! a successful registration the Windows Default apps page is opened so the
//! user can confirm emusic for each type.
//!
//! Checkbox selection state lives in egui's own per-session memory (keyed
//! off this widget's id), not in [`crate::state::AppState`] or [`crate::app::EguiApp`]:
//! it's pure UI presentation with no effect until "Register" is pressed, so
//! it doesn't need to be threaded through the shell or persisted to disk.

use eframe::egui;
use winshell::assoc::{AssocManager, EXTENSIONS, open_default_apps_settings};

const APP_NAME: &str = "emusic";

pub fn show(ui: &mut egui::Ui) {
    ui.label("File associations");
    ui.label(
        egui::RichText::new(
            "Choose which audio file types emusic should open, then Register. \
             Windows 10/11 does not let an app make itself the default, so \
             Settings opens for you to confirm emusic for each type.",
        )
        .weak(),
    );
    ui.add_space(8.0);

    let manager = AssocManager::new(APP_NAME);
    let selection_id = ui.id().with("assoc_selection");
    let mut selected: Vec<bool> =
        ui.ctx()
            .data(|d| d.get_temp(selection_id))
            .unwrap_or_else(|| {
                EXTENSIONS
                    .iter()
                    .map(|ext| manager.is_registered(ext))
                    .collect()
            });

    ui.horizontal(|ui| {
        if ui.small_button("Select all").clicked() {
            selected.iter_mut().for_each(|on| *on = true);
        }
        if ui.small_button("Select none").clicked() {
            selected.iter_mut().for_each(|on| *on = false);
        }
    });
    ui.add_space(4.0);

    egui::Grid::new(ui.id().with("assoc_grid"))
        .num_columns(4)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for (i, ext) in EXTENSIONS.iter().enumerate() {
                ui.checkbox(&mut selected[i], format!(".{ext}"));
                if (i + 1) % 4 == 0 {
                    ui.end_row();
                }
            }
        });

    ui.add_space(8.0);

    let status_id = ui.id().with("assoc_status");
    ui.horizontal(|ui| {
        if ui.button("Register").clicked() {
            let result = register_and_open_settings(&manager, &selected);
            refresh_selection(&manager, &mut selected);
            store_status(ui, status_id, result);
        }
        if ui.button("Unregister all").clicked() {
            let result = manager
                .unregister()
                .map(|()| "Removed all emusic file associations.".to_string())
                .map_err(|e| e.to_string());
            refresh_selection(&manager, &mut selected);
            store_status(ui, status_id, result);
        }
        if ui.button("Open Windows Default Apps...").clicked() {
            let result = open_default_apps_settings(APP_NAME)
                .map(|()| "Opened Windows Default Apps.".to_string())
                .map_err(|e| e.to_string());
            store_status(ui, status_id, result);
        }
    });

    // Persist the (possibly action-updated) selection for the next frame.
    ui.ctx().data_mut(|d| d.insert_temp(selection_id, selected));

    if let Some((message, is_error)) = ui.ctx().data(|d| d.get_temp::<(String, bool)>(status_id)) {
        let color = if is_error {
            ui.visuals().error_fg_color
        } else {
            ui.visuals().weak_text_color()
        };
        ui.colored_label(color, message);
    }
}

/// Registers the selected extensions and, on success, opens Windows' Default
/// apps page so the user can make emusic the default.
fn register_and_open_settings(manager: &AssocManager, selected: &[bool]) -> Result<String, String> {
    let count = register_selected(manager, selected)?;
    let message = format!(
        "Registered {count} file type{}. Choose emusic in Windows Settings to make it the default.",
        if count == 1 { "" } else { "s" }
    );
    match open_default_apps_settings(APP_NAME) {
        Ok(()) => Ok(message),
        Err(err) => Ok(format!("{message} (could not open Settings: {err})")),
    }
}

/// Registers the checked extensions, returning how many were registered.
fn register_selected(manager: &AssocManager, selected: &[bool]) -> Result<usize, String> {
    let exts: Vec<&str> = EXTENSIONS
        .iter()
        .zip(selected)
        .filter_map(|(ext, &on)| on.then_some(*ext))
        .collect();
    if exts.is_empty() {
        return Err("Select at least one file type to register.".to_string());
    }

    let exe = std::env::current_exe().map_err(|e| format!("could not locate emusic.exe: {e}"))?;
    manager.register(&exe, &exts).map_err(|e| e.to_string())?;
    Ok(exts.len())
}

/// Re-reads the actual registration state into the checkbox selection, so the
/// UI reflects what a Register/Unregister action just changed.
fn refresh_selection(manager: &AssocManager, selected: &mut [bool]) {
    for (on, ext) in selected.iter_mut().zip(EXTENSIONS) {
        *on = manager.is_registered(ext);
    }
}

/// Records the outcome of a button action in egui's temp memory so the next
/// frame can render it.
fn store_status(ui: &egui::Ui, id: egui::Id, result: Result<String, String>) {
    match result {
        Ok(message) => {
            ui.ctx().data_mut(|d| d.insert_temp(id, (message, false)));
        }
        Err(err) => {
            tracing::warn!(%err, "file association action failed");
            ui.ctx().data_mut(|d| d.insert_temp(id, (err, true)));
        }
    }
}
