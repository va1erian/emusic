//! Shared vector icons drawn with [`egui::Painter`] (#60, #81).
//!
//! Text glyphs (`▶`, `⏸`, …) come from a fallback font whose metrics do not
//! line up with the surrounding layout, leaving the play triangle visibly
//! off-centre (once against the transport button box, once against the track
//! table's row). Painting the shapes ourselves keeps every icon centred and
//! crisp at any DPI: each function draws into a square icon box derived from
//! the [`egui::Rect`] it is given, so the caller only has to pass that rect
//! and the current colour.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// Fraction of the button's shorter side used as the icon's bounding box.
/// Kept below 1 so there is always a little breathing room around the icon.
const ICON_SCALE: f32 = 0.56;

/// Extra rightward nudge (as a fraction of the icon width) applied to the
/// play triangle. A right-pointing triangle carries more visual weight on
/// its base, so a purely geometric centre looks shifted left; nudging it
/// right puts the triangle's optical centre back on the button centre.
const PLAY_OPTICAL_SHIFT: f32 = 0.12;

/// A square icon box centred in `rect`, with a side of `scale` times the
/// shorter side of `rect`.
fn icon_box(rect: Rect, scale: f32) -> Rect {
    let side = rect.width().min(rect.height()) * scale;
    Rect::from_center_size(rect.center(), Vec2::splat(side))
}

/// The three corners of a right-pointing play triangle filling the square
/// `box`, optically centred.
fn play_triangle_in(box_: Rect) -> [Pos2; 3] {
    let shift = box_.width() * PLAY_OPTICAL_SHIFT;
    let left = box_.left() + shift;
    let right = box_.right() + shift;
    let mid_y = box_.center().y;
    [
        egui::pos2(left, box_.top()),
        egui::pos2(right, mid_y),
        egui::pos2(left, box_.bottom()),
    ]
}

/// The three corners of the right-pointing play triangle, optically centred
/// in the icon box derived from `rect`.
fn play_triangle(rect: Rect) -> [Pos2; 3] {
    play_triangle_in(icon_box(rect, ICON_SCALE))
}

fn paint_triangle(painter: &Painter, points: [Pos2; 3], color: Color32) {
    painter.add(Shape::convex_polygon(points.to_vec(), color, Stroke::NONE));
}

/// Right-pointing triangle: play.
pub(crate) fn play(painter: &Painter, rect: Rect, color: Color32) {
    paint_triangle(painter, play_triangle(rect), color);
}

/// Right-pointing triangle filling `box_` exactly: the inline "now playing"
/// marker drawn by the track table, sized and positioned by the caller so it
/// can centre it on the row rather than on the text baseline.
pub(crate) fn play_in(painter: &Painter, box_: Rect, color: Color32) {
    paint_triangle(painter, play_triangle_in(box_), color);
}

/// Two vertical bars: pause.
pub(crate) fn pause(painter: &Painter, rect: Rect, color: Color32) {
    let b = icon_box(rect, 0.5);
    let bar_width = b.width() * 0.3;
    let radius = bar_width * 0.2;
    let height = b.height();
    let left = Rect::from_min_size(egui::pos2(b.left(), b.top()), Vec2::new(bar_width, height));
    let right = Rect::from_min_size(
        egui::pos2(b.right() - bar_width, b.top()),
        Vec2::new(bar_width, height),
    );
    painter.rect_filled(left, radius, color);
    painter.rect_filled(right, radius, color);
}

/// A filled square: stop.
pub(crate) fn stop(painter: &Painter, rect: Rect, color: Color32) {
    let b = icon_box(rect, 0.46);
    painter.rect_filled(b, b.width() * 0.1, color);
}

/// A left-pointing triangle next to a bar: previous track.
pub(crate) fn previous(painter: &Painter, rect: Rect, color: Color32) {
    let b = icon_box(rect, 0.58);
    let bar_width = b.width() * 0.16;
    let bar = Rect::from_min_size(
        egui::pos2(b.left(), b.top()),
        Vec2::new(bar_width, b.height()),
    );
    painter.rect_filled(bar, bar_width * 0.2, color);

    let tip_x = b.left() + bar_width + b.width() * 0.06;
    painter.add(Shape::convex_polygon(
        vec![
            egui::pos2(b.right(), b.top()),
            egui::pos2(b.right(), b.bottom()),
            egui::pos2(tip_x, b.center().y),
        ],
        color,
        Stroke::NONE,
    ));
}

/// A right-pointing triangle next to a bar: next track.
pub(crate) fn next(painter: &Painter, rect: Rect, color: Color32) {
    let b = icon_box(rect, 0.58);
    let bar_width = b.width() * 0.16;
    let bar = Rect::from_min_size(
        egui::pos2(b.right() - bar_width, b.top()),
        Vec2::new(bar_width, b.height()),
    );
    painter.rect_filled(bar, bar_width * 0.2, color);

    let tip_x = b.right() - bar_width - b.width() * 0.06;
    painter.add(Shape::convex_polygon(
        vec![
            egui::pos2(b.left(), b.top()),
            egui::pos2(b.left(), b.bottom()),
            egui::pos2(tip_x, b.center().y),
        ],
        color,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button_rect() -> Rect {
        Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(30.0, 30.0))
    }

    #[test]
    fn icon_box_is_centred_and_square() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(40.0, 24.0));
        let b = icon_box(rect, ICON_SCALE);
        assert_eq!(b.center(), rect.center());
        assert!((b.width() - b.height()).abs() < 1e-3);
        assert!((b.width() - 24.0 * ICON_SCALE).abs() < 1e-3);
    }

    #[test]
    fn play_triangle_is_nudged_right_and_inside_the_button() {
        let rect = button_rect();
        let triangle = play_triangle(rect);
        let centroid = |points: [Pos2; 3]| points.iter().map(|p| p.x).sum::<f32>() / 3.0;

        // Optically centred: shifted right of a purely geometric centring,
        // but never as far as aligning the centroid with the button centre.
        let geometric = [triangle[0] - Vec2::new(2.0, 0.0), triangle[1], triangle[2]];
        let width = icon_box(rect, ICON_SCALE).width();
        assert!(centroid(triangle) > centroid(geometric));
        assert!(centroid(triangle) < rect.center().x);
        assert!(rect.center().x - centroid(triangle) < width / 6.0);
        for p in triangle {
            assert!(rect.contains(p), "{p:?} escaped the button {rect:?}");
        }
    }

    #[test]
    fn play_triangle_in_is_vertically_centred_in_its_box() {
        let b = Rect::from_min_size(egui::pos2(4.0, 10.0), Vec2::splat(16.0));
        let t = play_triangle_in(b);
        assert_eq!(t[0].y, b.top());
        assert_eq!(t[1].y, b.center().y);
        assert_eq!(t[2].y, b.bottom());
        assert_eq!((t[0].y + t[2].y) / 2.0, b.center().y);
    }
}
