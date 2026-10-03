#![forbid(unsafe_code)]

//! The projectM surface's hover overlay (#302): a small row of buttons at the
//! top-right that pop the visualization out (or dock it back) or hide it.
//!
//! The buttons are owner-drawn while the surface uses its GDI fallback. A
//! [`Renderer::Gl`](win32ui::Renderer::Gl) surface cannot show native siblings
//! and the app forbids `unsafe`, so the GL overlay is blocked on the upstream
//! win32ui issue; once it lands, this layout and hit-testing are reused for it.

use win32ui::gdi::{Canvas, Font, TextFormat};
use win32ui::{Rect, Theme, dip};

/// What an overlay button asks the shell to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    /// Move the visualization to its other dock.
    PopOut,
    /// Hide it.
    Hide,
}

/// One laid-out overlay button, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Button {
    pub action: Action,
    pub rect: Rect,
}

/// Every overlay button, left to right.
const ACTIONS: [Action; 2] = [Action::PopOut, Action::Hide];

/// Button size and insets, in design units.
const WIDTH_DIP: f32 = 76.0;
const HEIGHT_DIP: f32 = 22.0;
const MARGIN_DIP: f32 = 6.0;
const GAP_DIP: f32 = 4.0;

/// Lays the buttons out along the top-right of `bounds`.
pub(super) fn buttons(bounds: Rect, dpi: u32) -> Vec<Button> {
    let width = dip(WIDTH_DIP).to_px(dpi).value();
    let height = dip(HEIGHT_DIP).to_px(dpi).value();
    let margin = dip(MARGIN_DIP).to_px(dpi).value();
    let gap = dip(GAP_DIP).to_px(dpi).value();
    let count = ACTIONS.len() as i32;
    let total = width * count + gap * (count - 1);
    let top = bounds.top + margin;
    let mut left = bounds.right - margin - total;
    let mut buttons = Vec::with_capacity(ACTIONS.len());
    for action in ACTIONS {
        buttons.push(Button {
            action,
            rect: Rect::new(left, top, left + width, top + height),
        });
        left += width + gap;
    }
    buttons
}

/// The action under `(x, y)`, if any.
pub(super) fn hit(buttons: &[Button], x: i32, y: i32) -> Option<Action> {
    buttons
        .iter()
        .find(|button| {
            let rect = button.rect;
            (rect.left..rect.right).contains(&x) && (rect.top..rect.bottom).contains(&y)
        })
        .map(|button| button.action)
}

/// Draws the buttons.
pub(super) fn draw(
    canvas: &Canvas,
    buttons: &[Button],
    theme: &Theme,
    font: Option<&Font>,
    dpi: u32,
) {
    let radius = dip(4.0).to_px(dpi).value();
    let format = TextFormat::left()
        .center()
        .vcenter()
        .single_line()
        .no_prefix();
    for button in buttons {
        canvas.round_rect(button.rect, radius, theme.raised, Some(theme.border));
        if let Some(font) = font {
            canvas.with_font(font, |canvas| {
                canvas.draw_text(button.rect, label(button.action), theme.text, format);
            });
        }
    }
}

/// The button's label.
fn label(action: Action) -> &'static str {
    match action {
        Action::PopOut => "Pop out",
        Action::Hide => "Hide",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_are_right_aligned_in_a_row() {
        let bounds = Rect::new(0, 0, 400, 300);
        let buttons = buttons(bounds, 96);
        assert_eq!(
            buttons.iter().map(|b| b.action).collect::<Vec<_>>(),
            vec![Action::PopOut, Action::Hide]
        );
        // Right edge sits at the margin, and every button shares the top.
        let last = buttons.last().expect("two buttons");
        assert_eq!(last.rect.right, 400 - 6);
        assert!(buttons.iter().all(|b| b.rect.top == 6));
        // Laid left-to-right.
        assert!(buttons[0].rect.left < buttons[1].rect.left);
    }

    #[test]
    fn hit_finds_the_button_under_the_point() {
        let bounds = Rect::new(0, 0, 400, 300);
        let buttons = buttons(bounds, 96);
        let hide = buttons[1].rect;
        let centre = ((hide.left + hide.right) / 2, (hide.top + hide.bottom) / 2);
        assert_eq!(hit(&buttons, centre.0, centre.1), Some(Action::Hide));
        assert_eq!(hit(&buttons, 0, 0), None);
    }

    #[test]
    fn hit_ignores_the_gaps_between_buttons() {
        let bounds = Rect::new(0, 0, 400, 300);
        let buttons = buttons(bounds, 96);
        // Just right of the pop-out button, inside the gap.
        let gap_x = buttons[0].rect.right + 1;
        assert_eq!(hit(&buttons, gap_x, buttons[0].rect.top + 1), None);
    }
}
