//! Win32 navigator (#109): a self-drawn list of the shared view sections
//! (`emusic_ui::panels::navigator`), each row an icon (Segoe Fluent Icons)
//! plus the view label, with a selection highlight.
//!
//! The sections and the click-to-switch intent come from the model; this view
//! only owns the child window and draws it.

use std::cell::{Cell, RefCell};

use emusic_ui::panels::navigator::{Navigator, SECTIONS};
use emusic_ui::state::View;
use win32ui::gdi::{Canvas, Font, FontWeight, TextFormat};
use win32ui::prelude::*;
use win32ui::{Custom, CustomWidget, Input, Rect, Size, Theme, WidgetCx};

use crate::app::Msg;

/// Row height, in device-independent pixels.
const ROW_HEIGHT: f32 = 26.0;
/// Section-heading row height.
const HEADING_HEIGHT: f32 = 24.0;
/// Gap after each section.
const SECTION_GAP: f32 = 8.0;
/// Top padding before the first heading.
const TOP_PAD: f32 = 4.0;
/// Left inset of an icon.
const ICON_LEFT: f32 = 10.0;
/// Icon box size.
const ICON_SIZE: f32 = 16.0;
/// Left inset of a row's label (after the icon).
const LABEL_LEFT: f32 = 34.0;
/// Left inset of a section heading.
const HEADING_LEFT: f32 = 12.0;
/// Initial width the layout usually overrides, in device-independent pixels.
const INITIAL_WIDTH: f32 = 200.0;
/// Initial height, in device-independent pixels.
const INITIAL_HEIGHT: f32 = 300.0;

/// The event the navigator raises when a view is clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigatorEvent {
    /// The user clicked this view's row.
    Select(View),
}

/// A navigator row: a non-selectable section heading, or a view.
enum Row<'a> {
    Heading(&'a str),
    View(View),
}

/// The Win32 navigator: a child window drawing the shared view list.
pub struct NavigatorView {
    custom: Custom<NavigatorWidget, Msg>,
    applied_revision: Cell<u64>,
}

impl NavigatorView {
    /// Creates the navigator and maps its clicks to [`Msg::Navigate`].
    pub fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let widget = NavigatorWidget::new(ui.dpi());
        let custom = Custom::new(ui, widget)?.on_event(|event| Some(Msg::Navigate(event.view())));
        Ok(Self {
            custom,
            applied_revision: Cell::new(u64::MAX),
        })
    }

    /// Updates the highlighted row, repainting only when the selection changed.
    pub fn sync(&self, navigator: &Navigator) {
        if self.applied_revision.get() == navigator.revision() {
            return;
        }
        self.applied_revision.set(navigator.revision());
        self.custom
            .widget()
            .borrow_mut()
            .set_selected(navigator.selected());
        self.custom.invalidate();
    }
}

impl AsControl for NavigatorView {
    fn control(&self) -> &Control {
        self.custom.control()
    }
}

impl NavigatorEvent {
    /// The view that was clicked.
    fn view(self) -> View {
        match self {
            NavigatorEvent::Select(view) => view,
        }
    }
}

/// The owner-drawn widget: the sections, the selected view and the hover.
struct NavigatorWidget {
    dpi: Cell<u32>,
    selected: Cell<View>,
    hot: Cell<Option<View>>,
    pressed: Cell<Option<View>>,
    body: RefCell<Option<Font>>,
    icons: RefCell<Option<Font>>,
}

impl NavigatorWidget {
    fn new(dpi: u32) -> Self {
        Self {
            dpi: Cell::new(dpi),
            selected: Cell::new(View::default()),
            hot: Cell::new(None),
            pressed: Cell::new(None),
            body: RefCell::new(Font::new("Segoe UI", 9.75, FontWeight::Regular, dpi).ok()),
            icons: RefCell::new(
                Font::new("Segoe Fluent Icons", 11.0, FontWeight::Regular, dpi).ok(),
            ),
        }
    }

    fn set_selected(&mut self, view: View) {
        if self.selected.get() != view {
            self.selected.set(view);
        }
    }

    /// The view under a client point, if any (headings are not selectable).
    fn hit(&self, x: i32, y: i32, bounds: Rect) -> Option<View> {
        let mut found = None;
        for_each_row(self.dpi.get(), bounds, |row, rect| {
            if let Row::View(view) = row
                && rect.contains(Point::new(x, y))
            {
                found = Some(view);
            }
        });
        found
    }

    /// The title-bar icon for a view (Segoe Fluent Icons codepoint).
    fn glyph(view: View) -> &'static str {
        match view {
            View::Music => "\u{E8D6}",      // Audio
            View::Albums => "\u{E93C}",     // MusicAlbum
            View::Artists => "\u{E716}",    // People
            View::Genres => "\u{E8EC}",     // Tag
            View::Folders => "\u{E8B7}",    // Folder
            View::Starred => "\u{E734}",    // FavoriteStar
            View::MostPlayed => "\u{E735}", // FavoriteStarFill
            View::History => "\u{E81C}",    // History
            View::NowPlaying => "\u{E768}", // Play
            View::Settings => "\u{E713}",   // Settings
        }
    }
}

impl CustomWidget for NavigatorWidget {
    type Event = NavigatorEvent;

    fn preferred_size(&self, dpi: u32) -> Option<Size> {
        self.dpi.set(dpi);
        Some(Size::new(
            dip(INITIAL_WIDTH).to_px(dpi).value(),
            dip(INITIAL_HEIGHT).to_px(dpi).value(),
        ))
    }

    fn paint(&self, canvas: &Canvas, bounds: Rect, theme: &Theme) {
        canvas.fill_rect(bounds, theme.background);
        let selected = self.selected.get();
        let hot = self.hot.get();
        let scale = self.dpi.get() as f32 / 96.0;
        let icon_left = (ICON_LEFT * scale).round() as i32;
        let icon_size = (ICON_SIZE * scale).round() as i32;
        let label_left = (LABEL_LEFT * scale).round() as i32;
        let heading_left = (HEADING_LEFT * scale).round() as i32;
        let body = self.body.borrow();
        let icons = self.icons.borrow();

        for_each_row(self.dpi.get(), bounds, |row, rect| match row {
            Row::Heading(text) => {
                let Some(font) = body.as_ref() else {
                    return;
                };
                let text_rect =
                    Rect::new(rect.left + heading_left, rect.top, rect.right, rect.bottom);
                let format = TextFormat::left().vcenter().single_line().no_prefix();
                canvas.with_font(font, |canvas| {
                    canvas.draw_text(text_rect, text, theme.text_secondary, format);
                });
            }
            Row::View(view) => {
                if view == selected {
                    canvas.fill_rect(rect, theme.selection);
                } else if hot == Some(view) {
                    canvas.fill_rect(rect, theme.hover);
                }
                if let Some(font) = icons.as_ref() {
                    let icon_rect = Rect::new(
                        rect.left + icon_left,
                        rect.top,
                        rect.left + icon_left + icon_size,
                        rect.bottom,
                    );
                    let color = if view == selected {
                        theme.accent
                    } else {
                        theme.text_secondary
                    };
                    let format = TextFormat::left()
                        .center()
                        .vcenter()
                        .single_line()
                        .no_prefix();
                    canvas.with_font(font, |canvas| {
                        canvas.draw_text(icon_rect, Self::glyph(view), color, format);
                    });
                }
                let Some(font) = body.as_ref() else {
                    return;
                };
                let text_rect =
                    Rect::new(rect.left + label_left, rect.top, rect.right, rect.bottom);
                let format = TextFormat::left()
                    .vcenter()
                    .single_line()
                    .end_ellipsis()
                    .no_prefix();
                canvas.with_font(font, |canvas| {
                    canvas.draw_text(text_rect, view.label(), theme.text, format);
                });
            }
        });
    }

    fn input(&self, input: Input, cx: &mut WidgetCx<NavigatorEvent>) {
        let bounds = cx.bounds();
        match input {
            Input::MouseMove { x, y } => {
                let hit = self.hit(x, y, bounds);
                if hit != self.hot.get() {
                    self.hot.set(hit);
                    cx.invalidate();
                }
            }
            Input::MouseLeave => {
                if self.hot.take().is_some() {
                    cx.invalidate();
                }
            }
            Input::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                self.pressed.set(self.hit(x, y, bounds));
            }
            Input::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let hit = self.hit(x, y, bounds);
                if hit.is_some()
                    && hit == self.pressed.take()
                    && let Some(view) = hit
                {
                    cx.emit(NavigatorEvent::Select(view));
                }
            }
            _ => {}
        }
    }
}

/// Walks the rows of `bounds`, calling `f` with each row and its rectangle (in
/// the widget's client coordinates), without allocating.
fn for_each_row<'a>(dpi: u32, bounds: Rect, mut f: impl FnMut(Row<'a>, Rect)) {
    let scale = dpi as f32 / 96.0;
    let row_h = (ROW_HEIGHT * scale).round() as i32;
    let heading_h = (HEADING_HEIGHT * scale).round() as i32;
    let gap = (SECTION_GAP * scale).round() as i32;
    let pad = (TOP_PAD * scale).round() as i32;
    let mut y = bounds.top + pad;
    for section in SECTIONS {
        f(
            Row::Heading(section.heading),
            Rect::new(bounds.left, y, bounds.right, y + heading_h),
        );
        y += heading_h;
        for &view in section.views {
            f(
                Row::View(view),
                Rect::new(bounds.left, y, bounds.right, y + row_h),
            );
            y += row_h;
        }
        y += gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_cover_every_view_in_section_order() {
        let mut views = Vec::new();
        for_each_row(96, Rect::new(0, 0, 200, 400), |row, _| {
            if let Row::View(view) = row {
                views.push(view);
            }
        });
        let expected: Vec<View> = SECTIONS
            .iter()
            .flat_map(|section| section.views.iter().copied())
            .collect();
        assert_eq!(views, expected);
    }

    #[test]
    fn hit_test_finds_the_row_under_the_point() {
        let widget = NavigatorWidget::new(96);
        let bounds = Rect::new(0, 0, 200, 400);
        let mut music_rect = Rect::default();
        for_each_row(96, bounds, |row, rect| {
            if matches!(row, Row::View(View::Music)) {
                music_rect = rect;
            }
        });
        let center = Point::new(
            music_rect.left + 5,
            (music_rect.top + music_rect.bottom) / 2,
        );
        assert_eq!(widget.hit(center.x, center.y, bounds), Some(View::Music));
        // A heading row is not selectable.
        assert_eq!(widget.hit(2, 2, bounds), None);
    }
}
