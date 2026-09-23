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

/// Like [`ICON_SCALE`] but for the favourite star, which is drawn larger so
/// it reads clearly at row size (#193).
const STAR_SCALE: f32 = 0.72;

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

/// Fraction of the star's outer radius used for its five inner (notch)
/// vertices. A little above the geometric 0.38 so the star reads as plump
/// rather than spiky at table-row size.
const STAR_INNER_RATIO: f32 = 0.42;

/// How far the star's visual centre sits below its bounding-box centre, as a
/// fraction of the outer radius. The top point reaches a full radius above
/// the centre but the bottom points only `cos(36 deg)` of one below it, so
/// the star has to be shifted down by half the difference to look centred.
const STAR_CENTER_DROP: f32 = (1.0 - 0.809_017) / 2.0;

/// The centre of the star drawn in `box_`, nudged down so the star looks
/// vertically centred in the box.
fn star_center(box_: Rect) -> Pos2 {
    let outer = box_.width().min(box_.height()) * 0.5;
    let center = box_.center();
    egui::pos2(center.x, center.y + outer * STAR_CENTER_DROP)
}

/// The ten alternating outer/inner vertices of a five-pointed star filling
/// the square `box_`, with the top point straight up.
fn star_vertices(box_: Rect) -> Vec<Pos2> {
    let center = star_center(box_);
    let outer = box_.width().min(box_.height()) * 0.5;
    let inner = outer * STAR_INNER_RATIO;
    (0..10)
        .map(|i| {
            let radius = if i % 2 == 0 { outer } else { inner };
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            egui::pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

/// A filled five-pointed star: the track table's starred marker (#131).
///
/// A star is concave, so it is filled as a fan of ten triangles around its
/// centre (valid because the centre can see every vertex) rather than with a
/// single [`Shape::convex_polygon`].
pub(crate) fn star(painter: &Painter, rect: Rect, color: Color32) {
    let box_ = icon_box(rect, STAR_SCALE);
    let center = star_center(box_);
    let points = star_vertices(box_);
    for i in 0..points.len() {
        painter.add(Shape::convex_polygon(
            vec![center, points[i], points[(i + 1) % points.len()]],
            color,
            Stroke::NONE,
        ));
    }
}

/// The outline of a five-pointed star, drawn for the unstarred state so the
/// column keeps a visible, clickable target (#131).
pub(crate) fn star_outline(painter: &Painter, rect: Rect, color: Color32) {
    let box_ = icon_box(rect, STAR_SCALE);
    painter.add(Shape::closed_line(
        star_vertices(box_),
        Stroke::new(1.0, color),
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

    #[test]
    fn star_has_ten_alternating_points_facing_up() {
        let b = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::splat(20.0));
        let points = star_vertices(b);
        assert_eq!(points.len(), 10);

        let center = star_center(b);
        let radius = |p: Pos2| (p - center).length();
        // Even indices are the outer points, odd ones the inner notches.
        assert!(radius(points[0]) > radius(points[1]));
        assert!((radius(points[0]) - 10.0).abs() < 1e-3);
        // The first point is straight above the (nudged) centre.
        assert!((points[0].x - center.x).abs() < 1e-3);
        assert!((points[0].y - (center.y - 10.0)).abs() < 1e-3);
        for p in &points {
            assert!(b.expand(0.5).contains(*p), "{p:?} escaped {b:?}");
        }
        // Optically centred: the top and bottom extents are equidistant from
        // the box centre.
        let top = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let bottom = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(((top + bottom) / 2.0 - b.center().y).abs() < 1e-3);
    }
}
