//! Settings → Appearance page (#40, #115): the dark/light colour scheme, the
//! accent preset swatches, font size, list density, zebra striping, the window
//! accent tint (#355) and the status-bar visualizer (#25).
//!
//! Theme and accent changes go through [`Command`](emusic_ui::state::Command)s
//! so the shell stays the single writer of the shared state; the visualizer and
//! appearance flags are plain persisted state and are set directly.
//!
//! The portable [`RadioGroup`] stacks its options vertically, so each choice is
//! its own row rather than the Win32 build's horizontal strip.

use emusic_ui::state::{AppState, Density, FontSize, Theme, VisualizerMode};
use emusic_ui::views::Commands;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::WidgetId;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{CheckBox, ColorPicker, Label, RadioGroup, Slider};

use crate::app::Msg;

use super::SettingsMsg;
use super::accent_swatches::{self, STRIP_HEIGHT};
use super::form::{FormPage, HEADING_HEIGHT, LABEL_WIDTH, ROW_HEIGHT};
use crate::make::make;
use xui::xui_core::arrange::{checkbox, label, radio_group, slider};

/// The Appearance page's controls.
pub(super) struct AppearancePage {
    form: FormPage,
    _heading: Label<Msg>,
    _theme_label: Label<Msg>,
    theme: RadioGroup<Msg>,
    _font_label: Label<Msg>,
    font: RadioGroup<Msg>,
    _density_label: Label<Msg>,
    density: RadioGroup<Msg>,
    zebra: CheckBox<Msg>,
    _accent_label: Label<Msg>,
    accent: ColorPicker<Msg>,
    tint: CheckBox<Msg>,
    _tint_label: Label<Msg>,
    tint_strength: Slider<Msg>,
    visualizer: CheckBox<Msg>,
    _mode_label: Label<Msg>,
    mode: RadioGroup<Msg>,
}

impl AppearancePage {
    /// Builds the page's controls and maps them to [`SettingsMsg`]s.
    pub(super) fn new(ui: &Ui<Msg>) -> AppearancePage {
        let form = FormPage::new(ui).expect("create appearance form");

        let heading = make(form.ui(), label("Appearance")).expect("create appearance heading");
        form.add_full(heading.id(), HEADING_HEIGHT);

        let theme_label = make(form.ui(), label("Theme")).expect("create theme label");
        let choices = theme_choices();
        let theme = make(form.ui(), radio_group(&choices))
            .expect("create theme radios")
            .on_select(|index| {
                let message = match choice_at(index) {
                    ThemeChoice::System => SettingsMsg::SetFollowSystem(true),
                    ThemeChoice::Own(theme) => SettingsMsg::SetTheme(theme),
                };
                Some(Msg::Settings(message))
            });
        add_labelled_group(&form, theme_label.id(), &theme.ids());

        let font_label = make(form.ui(), label("Font size")).expect("create font label");
        let font = make(
            form.ui(),
            radio_group(&labels(FontSize::ALL.iter().map(|size| size.label()))),
        )
        .expect("create font radios")
        .on_select(|index| {
            FontSize::ALL
                .get(index)
                .copied()
                .map(|size| Msg::Settings(SettingsMsg::SetFontSize(size)))
        });
        add_labelled_group(&form, font_label.id(), &font.ids());

        let density_label = make(form.ui(), label("List density")).expect("create density label");
        let density = make(
            form.ui(),
            radio_group(&labels(Density::ALL.iter().map(|density| density.label()))),
        )
        .expect("create density radios")
        .on_select(|index| {
            Density::ALL
                .get(index)
                .copied()
                .map(|density| Msg::Settings(SettingsMsg::SetDensity(density)))
        });
        add_labelled_group(&form, density_label.id(), &density.ids());

        let zebra = make(form.ui(), checkbox("Zebra striping"))
            .expect("create zebra checkbox")
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::ToggleZebra(on))));
        form.add_full(zebra.id(), ROW_HEIGHT);

        let accent_label = make(form.ui(), label("Accent colour")).expect("create accent label");
        let accent = accent_swatches::create(form.ui());
        form.add_full(accent_label.id(), ROW_HEIGHT);
        form.add_full(accent.id(), dip(STRIP_HEIGHT));

        let tint = make(form.ui(), checkbox("Tint acrylic bands with the accent"))
            .expect("create tint checkbox")
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::SetAccentTint(on))));
        form.add_full(tint.id(), ROW_HEIGHT);

        let tint_row = form.row(ROW_HEIGHT).expect("create tint row");
        let tint_label = make(tint_row.ui(), label("Tint strength")).expect("create tint label");
        let tint_strength = make(tint_row.ui(), slider(0.0, 255.0))
            .expect("create tint slider")
            .on_change(|value| {
                Some(Msg::Settings(SettingsMsg::SetAccentTintStrength(
                    value.round().clamp(0.0, 255.0) as u8,
                )))
            });
        tint_row
            .fixed(tint_label.id(), LABEL_WIDTH)
            .fill(tint_strength.id())
            .finish();

        let visualizer = make(form.ui(), checkbox("Visualizer"))
            .expect("create visualizer checkbox")
            .on_toggle(|on| Some(Msg::Settings(SettingsMsg::ToggleVisualizer(on))));
        form.add_full(visualizer.id(), ROW_HEIGHT);

        let mode_label = make(form.ui(), label("Visualizer mode")).expect("create mode label");
        let mode = make(
            form.ui(),
            radio_group(&labels(VisualizerMode::ALL.iter().map(|mode| mode.label()))),
        )
        .expect("create mode radios")
        .on_select(|index| {
            VisualizerMode::ALL
                .get(index)
                .copied()
                .map(|mode| Msg::Settings(SettingsMsg::SetVisualizerMode(mode)))
        });
        add_labelled_group(&form, mode_label.id(), &mode.ids());

        AppearancePage {
            form,
            _heading: heading,
            _theme_label: theme_label,
            theme,
            _font_label: font_label,
            font,
            _density_label: density_label,
            density,
            zebra,
            _accent_label: accent_label,
            accent,
            tint,
            _tint_label: tint_label,
            tint_strength,
            visualizer,
            _mode_label: mode_label,
            mode,
        }
    }

    /// The page's node identity (its scroll view).
    pub(super) fn id(&self) -> xui::xui_core::backend::WidgetId {
        self.form.id()
    }

    /// Re-lays the page's form.
    pub(super) fn relayout(&self) {
        self.form.relayout();
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.form.set_visible(visible);
    }

    /// Pushes the shared state onto the controls.
    pub(super) fn sync(&self, state: &AppState) {
        let following = follows_system(state);
        self.theme.select(choice_index(if following {
            ThemeChoice::System
        } else {
            ThemeChoice::Own(state.theme)
        }));
        self.font
            .select(index_of(&FontSize::ALL, state.appearance.font_size));
        self.density
            .select(index_of(&Density::ALL, state.appearance.density));
        self.zebra.set_checked(state.appearance.zebra);
        self.accent
            .select(accent_swatches::color(state.accent.rgb()));
        // The host's theme carries its own accent.
        self.form.ui().set_enabled(self.accent.id(), !following);
        self.tint.set_checked(state.accent_tint);
        self.tint_strength
            .set_value(f64::from(state.accent_tint_strength));
        self.tint_strength.set_enabled(state.accent_tint);
        self.visualizer.set_checked(state.visualizer_enabled);
        self.mode
            .select(index_of(&VisualizerMode::ALL, state.visualizer));
    }

    /// Handles the Appearance page's messages; returns whether `msg` was one.
    pub(super) fn update(
        &self,
        msg: &SettingsMsg,
        state: &mut AppState,
        out: &mut Commands,
    ) -> bool {
        match msg {
            SettingsMsg::SetTheme(theme) => {
                if follows_system(state) {
                    out.push(emusic_ui::state::Command::SetFollowSystem(false));
                }
                // The stored scheme mirrors the host's while following, so
                // this lands on the one picked either way.
                if state.theme != *theme {
                    out.push(emusic_ui::state::Command::ToggleTheme);
                }
            }
            SettingsMsg::SetFollowSystem(on) => {
                out.push(emusic_ui::state::Command::SetFollowSystem(*on));
            }
            SettingsMsg::SetAccentTint(on) => {
                out.push(emusic_ui::state::Command::SetAccentTint(*on));
                self.tint_strength.set_enabled(*on);
            }
            SettingsMsg::SetAccentTintStrength(strength) => {
                out.push(emusic_ui::state::Command::SetAccentTintStrength(*strength));
            }
            SettingsMsg::ToggleVisualizer(on) => state.visualizer_enabled = *on,
            SettingsMsg::SetVisualizerMode(mode) => state.visualizer = *mode,
            SettingsMsg::SetFontSize(size) => state.appearance.font_size = *size,
            SettingsMsg::SetDensity(density) => state.appearance.density = *density,
            SettingsMsg::ToggleZebra(on) => state.appearance.zebra = *on,
            _ => return false,
        }
        true
    }
}

/// Adds a group's label as a row, then one full-width row per option.
fn add_labelled_group(form: &FormPage, label: WidgetId, options: &[WidgetId]) {
    form.add_full(label, ROW_HEIGHT);
    for option in options {
        form.add_full(*option, ROW_HEIGHT);
    }
}

/// Collects `&str` labels into the slice [`RadioGroup::new`] wants.
fn labels<'a>(items: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    items.collect()
}

/// One choice in the theme radios.
#[derive(Clone, Copy, PartialEq)]
enum ThemeChoice {
    /// Follow the host's colour scheme and accent.
    System,
    /// emusic's own dark or light scheme.
    Own(Theme),
}

/// The theme radios' choices, in order: "Follow system" first, on a host
/// that has a theme to follow.
fn choices() -> Vec<ThemeChoice> {
    let mut all = Vec::with_capacity(3);
    if crate::backend::system_theme_offered() {
        all.push(ThemeChoice::System);
    }
    all.push(ThemeChoice::Own(Theme::Dark));
    all.push(ThemeChoice::Own(Theme::Light));
    all
}

/// The radios' labels.
fn theme_choices() -> Vec<&'static str> {
    choices()
        .into_iter()
        .map(|choice| match choice {
            ThemeChoice::System => "Follow system",
            ThemeChoice::Own(Theme::Dark) => "Dark",
            ThemeChoice::Own(Theme::Light) => "Light",
        })
        .collect()
}

/// The choice at radio `index`; the last one when out of range.
fn choice_at(index: usize) -> ThemeChoice {
    let all = choices();
    all.get(index)
        .or(all.last())
        .copied()
        .unwrap_or(ThemeChoice::Own(Theme::Dark))
}

/// The radio index of `choice`.
fn choice_index(choice: ThemeChoice) -> usize {
    index_of(&choices(), choice)
}

/// Whether the window follows the host's theme: the host has one, and the
/// user has not picked a scheme by hand.
fn follows_system(state: &AppState) -> bool {
    crate::backend::system_theme_offered() && state.follow_system.unwrap_or(true)
}

/// The position of `value` in `all`, or 0 when absent.
fn index_of<T: PartialEq>(all: &[T], value: T) -> usize {
    all.iter().position(|item| *item == value).unwrap_or(0)
}
