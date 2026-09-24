//! Settings → Appearance page (#40, #115): the dark/light colour scheme, the
//! accent presets and the optional status-bar visualizer.
//!
//! The theme and accent are applied through [`Command`]s so the shell stays the
//! single writer of the shared state; the visualizer flags are plain persisted
//! state (as in the egui view) and are set directly.

use std::cell::Cell;

use emusic_ui::state::{Accent, AppState, Theme as UiTheme, VisualizerMode};
use emusic_ui::views::Commands;
use win32ui::prelude::*;

use crate::app::Msg;

use super::{HEADING_HEIGHT, ROW_HEIGHT, SettingsMsg, labelled, radio_row};

/// The Appearance page's controls.
pub(super) struct AppearancePage {
    heading: Label,
    theme_label: Label,
    theme: RadioGroup<UiTheme, Msg>,
    accent_label: Label,
    accent: RadioGroup<Accent, Msg>,
    custom_note: Label,
    visualizer: CheckBox<Msg>,
    mode_label: Label,
    mode: RadioGroup<VisualizerMode, Msg>,
    /// Whether the page itself is currently shown.
    page_visible: Cell<bool>,
    /// Whether the visualizer is on (the mode strip only shows then).
    visualizer_on: Cell<bool>,
}

impl AppearancePage {
    /// Builds the page's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let theme = RadioGroup::new(ui, [("Dark", UiTheme::Dark), ("Light", UiTheme::Light)])?
            .selected(UiTheme::Dark)
            .on_select(|theme| Some(Msg::Settings(SettingsMsg::SetTheme(*theme))));

        let accent = RadioGroup::new(
            ui,
            Accent::PRESETS
                .into_iter()
                .map(|accent| (accent.label(), accent)),
        )?
        .on_select(|accent| Some(Msg::Settings(SettingsMsg::SetAccent(*accent))));

        let mode = RadioGroup::new(
            ui,
            VisualizerMode::ALL
                .into_iter()
                .map(|mode| (mode.label(), mode)),
        )?
        .on_select(|mode| Some(Msg::Settings(SettingsMsg::SetVisualizerMode(*mode))));

        let visualizer = CheckBox::new(ui, "Visualizer")?
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::ToggleVisualizer(on))));

        let page = Self {
            heading: Label::new(ui, Rect::default(), "Appearance")?,
            theme_label: Label::new(ui, Rect::default(), "Theme")?,
            theme,
            accent_label: Label::new(ui, Rect::default(), "Accent colour")?,
            accent,
            custom_note: Label::new(
                ui,
                Rect::default(),
                "Custom colours are not editable in the native frontend yet.",
            )?,
            visualizer,
            mode_label: Label::new(ui, Rect::default(), "Visualizer mode")?,
            mode,
            page_visible: Cell::new(false),
            visualizer_on: Cell::new(false),
        };
        page.refresh_mode_visibility();
        Ok(page)
    }

    /// The page's controls as layout items, in display order.
    pub(super) fn items(&self) -> Vec<LayoutItem> {
        vec![
            self.heading.height(dip(HEADING_HEIGHT)),
            labelled(
                &self.theme_label,
                radio_row(&self.theme).height(dip(ROW_HEIGHT)),
            ),
            labelled(
                &self.accent_label,
                radio_row(&self.accent).height(dip(ROW_HEIGHT)),
            ),
            self.custom_note.height(dip(ROW_HEIGHT)),
            self.visualizer.height(dip(ROW_HEIGHT)),
            labelled(
                &self.mode_label,
                radio_row(&self.mode).height(dip(ROW_HEIGHT)),
            ),
        ]
    }

    /// Shows or hides every control on the page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.page_visible.set(visible);
        self.heading.set_visible(visible);
        self.theme_label.set_visible(visible);
        self.theme.set_visible(visible);
        self.accent_label.set_visible(visible);
        self.accent.set_visible(visible);
        self.custom_note.set_visible(visible);
        self.visualizer.set_visible(visible);
        self.refresh_mode_visibility();
    }

    /// The visualizer mode strip only shows when both the page and the
    /// visualizer are on.
    fn refresh_mode_visibility(&self) {
        let visible = self.page_visible.get() && self.visualizer_on.get();
        self.mode_label.set_visible(visible);
        for option in self.mode.options() {
            option.set_visible(visible);
        }
    }

    /// Pushes the shared state onto the controls.
    pub(super) fn sync(&mut self, ui: &Ui<Msg>, state: &AppState) {
        self.theme.set_selected(&state.theme);
        self.accent.set_selected(&state.accent);
        self.visualizer.set_checked(state.visualizer_enabled);
        self.mode.set_selected(&state.visualizer);
        if self.visualizer_on.get() != state.visualizer_enabled {
            self.visualizer_on.set(state.visualizer_enabled);
            self.refresh_mode_visibility();
            ui.relayout();
        }
    }

    /// Handles the Appearance page's messages; returns whether `msg` was one.
    pub(super) fn update(
        &mut self,
        msg: &SettingsMsg,
        ui: &Ui<Msg>,
        state: &mut AppState,
        out: &mut Commands,
    ) -> bool {
        match msg {
            SettingsMsg::SetTheme(theme) => {
                if state.theme != *theme {
                    out.push(emusic_ui::state::Command::ToggleTheme);
                }
            }
            SettingsMsg::SetAccent(accent) => {
                out.push(emusic_ui::state::Command::SetAccent(*accent));
            }
            SettingsMsg::ToggleVisualizer(on) => {
                state.visualizer_enabled = *on;
                self.visualizer_on.set(*on);
                self.refresh_mode_visibility();
                ui.relayout();
            }
            SettingsMsg::SetVisualizerMode(mode) => state.visualizer = *mode,
            _ => return false,
        }
        true
    }
}
