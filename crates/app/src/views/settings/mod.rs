//! The Settings view (#376): the appearance and About pages ported to the
//! portable widget layer. The full tabbed Settings surface (library folders,
//! playback, associations, visualization) is a follow-up; this view exists so
//! the accent [`ColorPicker`](accent_swatches) and the About
//! [`FlowText`](about) have a real home and the `View::Settings` route works.
//!
//! The accent picker maps straight onto [`Command::SetAccent`]; the theme
//! button maps onto [`Command::ToggleTheme`].

pub mod about;
pub mod accent_swatches;

use emusic_ui::state::{Accent, Command, Theme};
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, ColorPicker, FlowText, Label, Tooltip};

use crate::app::Msg;
use about::FLOW_HEIGHT;
use accent_swatches::{STRIP_HEIGHT, color};

/// Outer margin of the page, in design units.
const MARGIN: f32 = 24.0;
/// Gap between blocks, in design units.
const GAP: f32 = 12.0;
/// Height of a section heading, in design units.
const HEADING_HEIGHT: f32 = 24.0;
/// Height of the theme button row, in design units.
const BUTTON_HEIGHT: f32 = 28.0;
/// Width of the theme button, in design units.
const BUTTON_WIDTH: f32 = 240.0;

/// The Settings view: appearance controls and the About text.
pub struct SettingsView {
    ui: Ui<Msg>,
    appearance_heading: Label<Msg>,
    theme: Button<Msg>,
    picker: ColorPicker<Msg>,
    about_heading: Label<Msg>,
    flow: FlowText<Msg>,
    // Kept alive so the hover tooltips keep working.
    _theme_tip: Tooltip<Msg>,
    _picker_tip: Tooltip<Msg>,
    /// The bounds the About flow was last built at. `FlowText` wraps to the
    /// size it was *created* with (a moved child gets no resize event), so it
    /// is rebuilt whenever its width would change.
    flow_bounds: Rect,
    /// Whether the whole page is shown, so a rebuilt flow inherits it.
    visible: bool,
    bounds: Rect,
}

impl SettingsView {
    /// Builds the view; call [`SettingsView::set_bounds`] to place it.
    pub fn new(ui: &Ui<Msg>) -> SettingsView {
        let appearance_heading =
            Label::new(ui, Rect::default(), "Appearance").expect("create appearance heading");
        let theme = Button::new(ui, Rect::default(), "Toggle dark / light theme")
            .expect("create theme button")
            .on_click(|| Some(Msg::Dispatch(Command::ToggleTheme)));
        let picker = accent_swatches::create(ui, Rect::default());
        let about_heading = Label::new(ui, Rect::default(), "About").expect("create about heading");
        let flow = about::build(ui, Rect::default());

        let theme_tip = Tooltip::attach(ui, theme.id(), "Switch between dark and light theme")
            .expect("attach theme tooltip");
        let picker_tip = Tooltip::attach(ui, picker.id(), "Choose the accent colour")
            .expect("attach picker tooltip");

        SettingsView {
            ui: ui.clone(),
            appearance_heading,
            theme,
            picker,
            about_heading,
            flow,
            _theme_tip: theme_tip,
            _picker_tip: picker_tip,
            flow_bounds: Rect::default(),
            visible: true,
            bounds: Rect::default(),
        }
    }

    /// Positions the page's controls inside `rect`.
    pub fn set_bounds(&mut self, rect: Rect) {
        self.bounds = rect;
        self.relayout();
    }

    fn relayout(&mut self) {
        let rect = self.bounds;
        let dpi = self.ui.dpi();
        let margin = dip(MARGIN).to_px(dpi).value();
        let gap = dip(GAP).to_px(dpi).value();
        let heading = dip(HEADING_HEIGHT).to_px(dpi).value();
        let button_h = dip(BUTTON_HEIGHT).to_px(dpi).value();
        let button_w = dip(BUTTON_WIDTH).to_px(dpi).value();
        let strip_h = dip(STRIP_HEIGHT).to_px(dpi).value();
        let flow_h = dip(FLOW_HEIGHT).to_px(dpi).value();
        let left = rect.left + margin;
        let right = rect.right - margin;

        let mut y = rect.top + margin;
        let mut moves = Vec::new();
        moves.push((
            self.appearance_heading.id(),
            Rect::new(left, y, right, y + heading),
        ));
        y += heading + gap;
        moves.push((
            self.theme.id(),
            Rect::new(left, y, (left + button_w).min(right), y + button_h),
        ));
        y += button_h + gap;
        moves.push((self.picker.id(), Rect::new(left, y, right, y + strip_h)));
        y += strip_h + 2 * gap;
        moves.push((
            self.about_heading.id(),
            Rect::new(left, y, right, y + heading),
        ));
        y += heading + gap;
        let flow_rect = Rect::new(left, y, right, y + flow_h);
        if flow_rect.width() != self.flow_bounds.width() {
            self.flow_bounds = flow_rect;
            self.flow = about::build(&self.ui, flow_rect);
            self.ui.set_visible(self.flow.id(), self.visible);
        }
        moves.push((self.flow.id(), flow_rect));
        self.ui.apply_moves(&moves);
    }

    /// Shows or hides the whole page.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        for id in [
            self.appearance_heading.id(),
            self.theme.id(),
            self.picker.id(),
            self.about_heading.id(),
            self.flow.id(),
        ] {
            self.ui.set_visible(id, visible);
        }
    }

    /// Mirrors the shell's accent and theme into the controls.
    pub fn sync(&self, accent: Accent, _theme: Theme) {
        self.picker.select(color(accent.rgb()));
    }
}
