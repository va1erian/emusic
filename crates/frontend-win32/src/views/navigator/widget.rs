//! The owner-drawn navigator widget (#109, #476): the shared view sections
//! followed by the runtime PLAYLISTS section, each row an icon (Segoe Fluent
//! Icons) plus a label, with a selection highlight, a `+` on the playlists
//! heading and a drop target for dragged tracks.

use std::cell::{Cell, RefCell};

use emusic_ui::library_api::PlaylistInfo;
use emusic_ui::state::{Metrics, View};
use win32ui::accessibility::{AccessCx, Action, Node, Role};
use win32ui::dnd::{DragEvent, DropEffect};
use win32ui::gdi::{Canvas, Font, FontWeight, TextFormat};
use win32ui::prelude::*;
use win32ui::{CustomWidget, Input, Point, Rect, Size, Theme, WidgetCx};

use super::NavigatorEvent;
use super::draw::{self, NodeKind, for_each_node};
use super::rows::{self, Row};

/// The face the navigator's icons are drawn with.
const ICON_FAMILY: &str = "Segoe Fluent Icons";
/// Base icon point size at [`FontSize::Default`](emusic_ui::state::FontSize::Default).
const ICON_POINTS: f32 = 11.0;
/// Initial width the layout usually overrides, in device-independent pixels.
const INITIAL_WIDTH: f32 = 200.0;
/// Initial height, in device-independent pixels.
const INITIAL_HEIGHT: f32 = 300.0;

/// The row under a client point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    /// The `+` on the playlists heading.
    Add,
    View(View),
    Playlist(u64),
}

/// The Win32 navigator's owner-drawn widget.
pub(super) struct NavigatorWidget {
    dpi: Cell<u32>,
    selected: Cell<View>,
    selected_playlist: Cell<Option<u64>>,
    playlists: RefCell<Vec<PlaylistInfo>>,
    hot: Cell<Option<Hit>>,
    pressed: Cell<Option<Hit>>,
    pressed_right: Cell<Option<Hit>>,
    /// The playlist row the keyboard acts on (F2 / Delete), set by clicking it.
    focused_playlist: Cell<Option<u64>>,
    /// The playlist row highlighted while tracks are dragged over it.
    hover_playlist: Cell<Option<u64>>,
    body: RefCell<Option<Font>>,
    icons: RefCell<Option<Font>>,
}

impl NavigatorWidget {
    pub(super) fn new(dpi: u32) -> Self {
        let widget = Self {
            dpi: Cell::new(dpi),
            selected: Cell::new(View::default()),
            selected_playlist: Cell::new(None),
            playlists: RefCell::new(Vec::new()),
            hot: Cell::new(None),
            pressed: Cell::new(None),
            pressed_right: Cell::new(None),
            focused_playlist: Cell::new(None),
            hover_playlist: Cell::new(None),
            body: RefCell::new(None),
            icons: RefCell::new(None),
        };
        widget.rebuild_fonts(crate::appearance::metrics());
        widget
    }

    /// Rebuilds the body and icon fonts from `metrics` at the current DPI.
    /// New fonts are created before the old handles are replaced, so a paint in
    /// flight can never touch a deleted `HFONT`.
    fn rebuild_fonts(&self, metrics: Metrics) {
        let dpi = self.dpi.get();
        *self.body.borrow_mut() = Font::new(
            crate::appearance::UI_FAMILY,
            crate::appearance::points(metrics.body),
            FontWeight::Regular,
            dpi,
        )
        .ok();
        // The glyph font tracks the text scale rather than the body size: the
        // icon box is sized from it in `paint`.
        let icon_points = ICON_POINTS * (metrics.body / crate::appearance::BASE_BODY_DIP);
        *self.icons.borrow_mut() =
            Font::new(ICON_FAMILY, icon_points, FontWeight::Regular, dpi).ok();
    }

    /// Applies new appearance metrics, rebuilding the fonts and repainting.
    pub(super) fn set_metrics(&mut self, metrics: Metrics, dpi: u32) {
        self.dpi.set(dpi);
        self.rebuild_fonts(metrics);
    }

    /// Adopts the shell's playlists, highlighted playlist and selected view.
    pub(super) fn set_state(
        &mut self,
        playlists: &[PlaylistInfo],
        selected_playlist: Option<u64>,
        selected: View,
    ) {
        self.playlists.replace(playlists.to_vec());
        self.selected.set(selected);
        self.selected_playlist.set(selected_playlist);
        // Drop the keyboard focus if that playlist is gone.
        if self
            .focused_playlist
            .get()
            .is_some_and(|id| !playlists.iter().any(|playlist| playlist.id == id))
        {
            self.focused_playlist.set(None);
        }
    }

    /// Whether playlist `id` is the highlighted row.
    fn is_highlighted(&self, id: u64) -> bool {
        self.selected.get() == View::Playlist && self.selected_playlist.get() == Some(id)
    }

    /// The row under a client point, if any (headings are not selectable).
    fn hit(&self, x: i32, y: i32, bounds: Rect) -> Option<Hit> {
        let point = Point::new(x, y);
        let mut found = None;
        rows::for_each_row(
            self.dpi.get(),
            bounds,
            &self.playlists.borrow(),
            |row, rect| match row {
                Row::Heading { add: true, .. }
                    if rows::add_button_rect(self.dpi.get(), rect).contains(point) =>
                {
                    found = Some(Hit::Add);
                }
                Row::View(view) if rect.contains(point) => found = Some(Hit::View(view)),
                Row::Playlist(info) if rect.contains(point) => {
                    found = Some(Hit::Playlist(info.id));
                }
                _ => {}
            },
        );
        found
    }

    /// The title-bar icon for a view (Segoe Fluent Icons codepoint).
    fn glyph(view: View) -> &'static str {
        match view {
            View::Music => "\u{E8D6}",         // Audio
            View::Albums => "\u{E93C}",        // MusicAlbum
            View::Artists => "\u{E716}",       // People
            View::Genres => "\u{E8EC}",        // Tag
            View::Folders => "\u{E8B7}",       // Folder
            View::Starred => "\u{E734}",       // FavoriteStar
            View::MostPlayed => "\u{E735}",    // FavoriteStarFill
            View::History => "\u{E81C}",       // History
            View::NowPlaying => "\u{E768}",    // Play
            View::Visualization => "\u{E7F4}", // TVMonitor
            View::Playlist => "\u{E8FD}",      // BulletedList
            View::Settings => "\u{E713}",      // Settings
        }
    }
}

impl CustomWidget for NavigatorWidget {
    type Event = NavigatorEvent;

    fn preferred_size(&self, dpi: u32) -> Option<Size> {
        if self.dpi.get() != dpi {
            self.dpi.set(dpi);
            self.rebuild_fonts(crate::appearance::metrics());
        }
        Some(Size::new(
            dip(INITIAL_WIDTH).to_px(dpi).value(),
            dip(INITIAL_HEIGHT).to_px(dpi).value(),
        ))
    }

    fn paint(&self, canvas: &Canvas, bounds: Rect, theme: &Theme) {
        canvas.fill_rect(bounds, theme.background);
        let selected = self.selected.get();
        let hot = self.hot.get();
        // Match `for_each_row`: the row bands and the insets grow with the
        // font-size setting, so the icon box always fits its larger glyph.
        let scale = self.dpi.get() as f32 / 96.0 * crate::appearance::font_scale();
        let label_left = (rows::LABEL_LEFT * scale).round() as i32;
        let heading_left = (rows::HEADING_LEFT * scale).round() as i32;
        let body = self.body.borrow();
        let icons = self.icons.borrow();

        rows::for_each_row(
            self.dpi.get(),
            bounds,
            &self.playlists.borrow(),
            |row, rect| match row {
                Row::Heading { text, add } => {
                    let Some(font) = body.as_ref() else {
                        return;
                    };
                    let text_rect =
                        Rect::new(rect.left + heading_left, rect.top, rect.right, rect.bottom);
                    let format = TextFormat::left().vcenter().single_line().no_prefix();
                    canvas.with_font(font, |canvas| {
                        canvas.draw_text(text_rect, text, theme.text_secondary, format);
                    });
                    if add && let Some(font) = icons.as_ref() {
                        let button = rows::add_button_rect(self.dpi.get(), rect);
                        let color = if hot == Some(Hit::Add) {
                            theme.text
                        } else {
                            theme.text_secondary
                        };
                        let format = TextFormat::left()
                            .center()
                            .vcenter()
                            .single_line()
                            .no_prefix();
                        canvas.with_font(font, |canvas| {
                            canvas.draw_text(button, "\u{E710}", color, format);
                        });
                    }
                }
                Row::View(view) => {
                    draw::paint_row(
                        canvas,
                        rect,
                        theme,
                        view == selected,
                        hot == Some(Hit::View(view)),
                    );
                    draw::paint_icon(
                        canvas,
                        rect,
                        theme,
                        icons.as_ref(),
                        Self::glyph(view),
                        view == selected,
                        scale,
                    );
                    draw::paint_label(canvas, rect, theme, body.as_ref(), view.label(), label_left);
                }
                Row::Playlist(info) => {
                    let highlighted = self.is_highlighted(info.id);
                    let hovering = self.hover_playlist.get() == Some(info.id);
                    draw::paint_row(
                        canvas,
                        rect,
                        theme,
                        highlighted || hovering,
                        hot == Some(Hit::Playlist(info.id)),
                    );
                    draw::paint_icon(
                        canvas,
                        rect,
                        theme,
                        icons.as_ref(),
                        Self::glyph(View::Playlist),
                        highlighted,
                        scale,
                    );
                    draw::paint_label(canvas, rect, theme, body.as_ref(), &info.name, label_left);
                }
            },
        );
    }

    fn accessibility(&self, cx: &AccessCx) -> Option<Node> {
        let mut rows = Vec::new();
        for_each_node(
            self.dpi.get(),
            cx.bounds(),
            &self.playlists.borrow(),
            |kind, rect| {
                rows.push(match kind {
                    NodeKind::Heading(text) => Node::new(Role::Text, text).bounds(rect),
                    NodeKind::View(view) => Node::new(Role::ListItem, view.label())
                        .id(format!(
                            "nav-{}",
                            view.label().to_lowercase().replace(' ', "-")
                        ))
                        .selected(view == self.selected.get())
                        .invokable()
                        .bounds(rect),
                    NodeKind::Add => Node::new(Role::Button, "New playlist")
                        .id("nav-playlist-add")
                        .invokable()
                        .bounds(rect),
                    NodeKind::Playlist(info) => Node::new(Role::ListItem, info.name.clone())
                        .id(format!("nav-playlist-{}", info.id))
                        .selected(self.is_highlighted(info.id))
                        .invokable()
                        .bounds(rect),
                });
            },
        );
        Some(Node::new(Role::List, "Navigator").children(rows))
    }

    fn accessibility_action(
        &self,
        path: &[usize],
        action: Action,
        cx: &mut WidgetCx<NavigatorEvent>,
    ) -> bool {
        let ([index], Action::Select | Action::Invoke) = (path, action) else {
            return false;
        };
        let mut found = None;
        let mut node_index = 0;
        let playlists = self.playlists.borrow();
        for_each_node(self.dpi.get(), cx.bounds(), &playlists, |kind, _| {
            if node_index == *index {
                found = Some(kind);
            }
            node_index += 1;
        });
        match found {
            Some(NodeKind::View(view)) => {
                cx.emit(NavigatorEvent::Select(view));
                true
            }
            Some(NodeKind::Playlist(info)) => {
                cx.emit(NavigatorEvent::SelectPlaylist(info.id));
                true
            }
            Some(NodeKind::Add) => {
                cx.emit(NavigatorEvent::AddPlaylist);
                true
            }
            _ => false,
        }
    }

    fn input(&self, input: Input, cx: &mut WidgetCx<NavigatorEvent>) {
        let bounds = cx.bounds();
        match input {
            Input::MouseMove { x, y, .. } => {
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
                ..
            } => {
                let hit = self.hit(x, y, bounds);
                self.pressed.set(hit);
                if hit.is_some() {
                    cx.focus();
                }
            }
            Input::MouseUp {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                let hit = self.hit(x, y, bounds);
                if hit.is_some() && hit == self.pressed.take() {
                    match hit {
                        Some(Hit::Add) => cx.emit(NavigatorEvent::AddPlaylist),
                        Some(Hit::View(view)) => cx.emit(NavigatorEvent::Select(view)),
                        Some(Hit::Playlist(id)) => {
                            self.focused_playlist.set(Some(id));
                            cx.emit(NavigatorEvent::SelectPlaylist(id));
                        }
                        None => {}
                    }
                }
            }
            Input::MouseDown {
                x,
                y,
                button: MouseButton::Right,
                ..
            } => {
                self.pressed_right.set(self.hit(x, y, bounds));
            }
            Input::MouseUp {
                x,
                y,
                button: MouseButton::Right,
                ..
            } => {
                let hit = self.hit(x, y, bounds);
                if hit.is_some() && hit == self.pressed_right.take() {
                    match hit {
                        Some(Hit::View(view)) => cx.emit(NavigatorEvent::Context(view)),
                        Some(Hit::Playlist(id)) => {
                            self.focused_playlist.set(Some(id));
                            cx.emit(NavigatorEvent::ContextPlaylist(id));
                        }
                        _ => {}
                    }
                }
            }
            Input::KeyDown { key, .. } => {
                let Some(id) = self.focused_playlist.get() else {
                    return;
                };
                match key {
                    Key::F2 => cx.emit(NavigatorEvent::RenamePlaylist(id)),
                    Key::DELETE => cx.emit(NavigatorEvent::DeletePlaylist(id)),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn drag(&self, event: DragEvent<'_>, cx: &mut WidgetCx<NavigatorEvent>) -> DropEffect {
        match event {
            DragEvent::Enter(info) | DragEvent::Over(info) => {
                let hover = match self.hit(info.x, info.y, cx.bounds()) {
                    Some(Hit::Playlist(id)) => Some(id),
                    _ => None,
                };
                if self.hover_playlist.get() != hover {
                    self.hover_playlist.set(hover);
                    cx.invalidate();
                }
                match (hover, info.data.has_payload()) {
                    (Some(_), true) => info.preferred_effect(),
                    _ => DropEffect::None,
                }
            }
            DragEvent::Leave => {
                if self.hover_playlist.take().is_some() {
                    cx.invalidate();
                }
                DropEffect::None
            }
            DragEvent::Drop(info) => {
                let hover = match self.hit(info.x, info.y, cx.bounds()) {
                    Some(Hit::Playlist(id)) => Some(id),
                    _ => None,
                };
                self.hover_playlist.take();
                let tracks = info
                    .data
                    .payload()
                    .and_then(|payload| crate::dnd::decode_track_ids(&payload));
                if let (Some(playlist), Some(tracks)) = (hover, tracks)
                    && !tracks.is_empty()
                {
                    cx.emit(NavigatorEvent::DropTracks { playlist, tracks });
                    return info.preferred_effect();
                }
                DropEffect::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_finds_playlist_rows_and_the_add_button() {
        let mut widget = NavigatorWidget::new(96);
        let lists = [PlaylistInfo {
            id: 7,
            name: "Mix".into(),
            track_count: 0,
        }];
        widget.set_state(&lists, Some(7), View::Playlist);
        let bounds = Rect::new(0, 0, 200, 800);
        let mut playlist = Rect::default();
        let mut add = Rect::default();
        rows::for_each_row(96, bounds, &lists, |row, rect| match row {
            Row::Heading { add: true, .. } => add = rows::add_button_rect(96, rect),
            Row::Playlist(_) => playlist = rect,
            _ => {}
        });

        let row_center = Point::new(playlist.left + 5, (playlist.top + playlist.bottom) / 2);
        assert_eq!(
            widget.hit(row_center.x, row_center.y, bounds),
            Some(Hit::Playlist(7))
        );
        let add_center = Point::new((add.left + add.right) / 2, (add.top + add.bottom) / 2);
        assert_eq!(
            widget.hit(add_center.x, add_center.y, bounds),
            Some(Hit::Add)
        );
    }
}
