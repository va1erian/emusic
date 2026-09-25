//! Settings → Appearance page (#40, #115): the dark/light colour scheme, the
//! accent preset swatches and custom colour, and the optional status-bar visualizer.
//!
//! The theme and accent are applied through [`Command`]s so the shell stays the
//! single writer of the shared state; the visualizer flags are plain persisted
//! state (as in the egui view) and are set directly.

use std::cell::Cell;

use emusic_ui::state::{Accent, AppState, Rgb, Theme as UiTheme, VisualizerMode};
use emusic_ui::views::Commands;
use win32ui::prelude::*;

use crate::app::Msg;

use super::accent_swatches::{self, AccentSwatches, STRIP_WIDTH};
use super::{FormRow, HEADING_HEIGHT, ROW_HEIGHT, ScrollPanel, SettingsMsg, labelled, radio_row};

/// Width of the custom-colour swatch button, in design units.
const CUSTOM_WIDTH: f32 = 72.0;

/// The Appearance page's controls.
pub(super) struct AppearancePage {
    form: ScrollPanel,
    heading: Label,
    theme_label: Label,
    theme: RadioGroup<UiTheme, Msg>,
    accent_label: Label,
    accent: Custom<AccentSwatches, Msg>,
    custom_label: Label,
    custom: ColorPicker<Msg>,
    visualizer: CheckBox<Msg>,
    mode_label: Label,
    mode: RadioGroup<VisualizerMode, Msg>,
    /// Whether the visualizer is on (the mode strip only shows then).
    visualizer_on: Cell<bool>,
}

impl AppearancePage {
    /// Builds the page's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let form = ScrollPanel::new(ui)?;
        let mut panel = form.ui(ui);
        let theme = RadioGroup::new(
            &mut panel,
            [("Dark", UiTheme::Dark), ("Light", UiTheme::Light)],
        )?
        .selected(UiTheme::Dark)
        .on_select(|theme| Some(Msg::Settings(SettingsMsg::SetTheme(*theme))));

        let accent = accent_swatches::create(&mut panel, |accent| {
            Some(Msg::Settings(SettingsMsg::SetAccent(accent)))
        })?;

        let mode = RadioGroup::new(
            &mut panel,
            VisualizerMode::ALL
                .into_iter()
                .map(|mode| (mode.label(), mode)),
        )?
        .on_select(|mode| Some(Msg::Settings(SettingsMsg::SetVisualizerMode(*mode))));

        let [r, g, b] = Accent::default().rgb().to_array();
        let custom = ColorPicker::new(&mut panel, Color::rgb(r, g, b))?.on_change(|color| {
            let rgb = Rgb::from_rgb(color.r, color.g, color.b);
            Some(Msg::Settings(SettingsMsg::SetAccent(Accent::Custom(rgb))))
        });

        let visualizer = CheckBox::new(&mut panel, "Visualizer")?
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::ToggleVisualizer(on))));

        let page = Self {
            form,
            heading: Label::new(&mut panel, Rect::default(), "Appearance")?,
            theme_label: Label::new(&mut panel, Rect::default(), "Theme")?,
            theme,
            accent_label: Label::new(&mut panel, Rect::default(), "Accent colour")?,
            accent,
            custom_label: Label::new(&mut panel, Rect::default(), "Custom colour")?,
            custom,
            visualizer,
            mode_label: Label::new(&mut panel, Rect::default(), "Visualizer mode")?,
            mode,
            visualizer_on: Cell::new(false),
        };
        page.refresh_mode_visibility();
        page.apply(ui);
        Ok(page)
    }

    /// The page's scrollable form as one tab-strip page.
    pub(super) fn page(&self) -> LayoutItem {
        self.form.page()
    }

    /// The page's controls as form rows, in display order. The visualizer mode
    /// row is only part of the form while the visualizer is on.
    fn rows(&self) -> Vec<FormRow> {
        let mut rows = vec![
            (self.heading.height(dip(HEADING_HEIGHT)), HEADING_HEIGHT),
            (
                labelled(
                    &self.theme_label,
                    radio_row(&self.theme).height(dip(ROW_HEIGHT)),
                ),
                ROW_HEIGHT,
            ),
            (
                labelled(&self.accent_label, self.accent.width(dip(STRIP_WIDTH))),
                ROW_HEIGHT,
            ),
            (
                labelled(
                    &self.custom_label,
                    Layout::row().item(&self.custom).width(dip(CUSTOM_WIDTH)),
                ),
                ROW_HEIGHT,
            ),
            (self.visualizer.height(dip(ROW_HEIGHT)), ROW_HEIGHT),
        ];
        if self.visualizer_on.get() {
            rows.push((
                labelled(
                    &self.mode_label,
                    radio_row(&self.mode).height(dip(ROW_HEIGHT)),
                ),
                ROW_HEIGHT,
            ));
        }
        rows
    }

    /// Reinstalls the page's form (used after the mode row appears or hides).
    fn apply(&self, ui: &Ui<Msg>) {
        self.form.apply(ui, self.rows());
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.form.set_visible(visible);
    }

    /// The visualizer mode strip only shows while the visualizer is on. It is
    /// deliberately independent of the page's own visibility (a hidden page
    /// hides its children anyway): the layout skips hidden widgets, so
    /// hiding the options with the page left them unpositioned until the next
    /// resize.
    fn refresh_mode_visibility(&self) {
        let visible = self.visualizer_on.get();
        self.mode_label.set_visible(visible);
        for option in self.mode.options() {
            option.set_visible(visible);
        }
    }

    /// Pushes the shared state onto the controls.
    pub(super) fn sync(&mut self, ui: &Ui<Msg>, state: &AppState) {
        self.theme.set_selected(&state.theme);
        if self.accent.widget().borrow().select(state.accent) {
            self.accent.invalidate();
        }
        let [r, g, b] = state.accent.rgb().to_array();
        self.custom.set_color(Color::rgb(r, g, b));
        self.visualizer.set_checked(state.visualizer_enabled);
        self.mode.set_selected(&state.visualizer);
        if self.visualizer_on.get() != state.visualizer_enabled {
            self.visualizer_on.set(state.visualizer_enabled);
            self.refresh_mode_visibility();
            self.apply(ui);
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
                self.apply(ui);
            }
            SettingsMsg::SetVisualizerMode(mode) => state.visualizer = *mode,
            _ => return false,
        }
        true
    }
}
