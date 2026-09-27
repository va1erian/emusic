//! The Albums view's sort/size toolbar (#113): the album count, the sort
//! combo, the tile-size slider and the Shuffle / Close album buttons.
//!
//! It maps its controls to [`AlbumMsg`]s and mirrors the shared [`AlbumGrid`]
//! model's sort, size and selection onto them; the view owns the layout slot
//! and visibility, this module owns the controls.

use emusic_ui::views::album_grid::models::AlbumSort;
use emusic_ui::views::album_grid::{AlbumGrid, DEFAULT_TILE_SIZE, MAX_TILE_SIZE, MIN_TILE_SIZE};
use xui::xui_core::app::Ui;
use xui::xui_core::backend::Result;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, ComboBox, HasText, Label, Slider};

use super::AlbumMsg;
use crate::app::Msg;

/// The toolbar band height, in design units.
pub(super) const TOOLBAR_HEIGHT: f32 = 34.0;
/// Fixed toolbar control widths, in design units.
const COUNT_WIDTH: f32 = 92.0;
const SORT_LABEL_WIDTH: f32 = 34.0;
const SORT_WIDTH: f32 = 150.0;
const SIZE_LABEL_WIDTH: f32 = 34.0;
const SHUFFLE_WIDTH: f32 = 88.0;
const CLOSE_WIDTH: f32 = 104.0;
/// Toolbar horizontal gaps and the band inset, in design units.
const GAP: f32 = 6.0;
const INSET: f32 = 8.0;

/// The Albums view's toolbar controls.
pub(super) struct Toolbar {
    ui: Ui<Msg>,
    count: Label<Msg>,
    sort_label: Label<Msg>,
    sort: ComboBox<Msg>,
    size_label: Label<Msg>,
    size: Slider<Msg>,
    shuffle: Button<Msg>,
    close: Button<Msg>,
}

impl Toolbar {
    /// Creates the toolbar and maps each control to its [`AlbumMsg`].
    pub(super) fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        let sort = ComboBox::new(ui, Rect::default(), &sort_labels())?
            .on_select(|index| Some(Msg::Album(AlbumMsg::SetSort(AlbumSort::ALL[index]))));
        sort.select(sort_index(AlbumSort::default()));

        let size = Slider::new(
            ui,
            Rect::default(),
            f64::from(MIN_TILE_SIZE),
            f64::from(MAX_TILE_SIZE),
        )?
        .on_change(|value| Some(Msg::Album(AlbumMsg::SetTileSize(value as f32))));
        size.set_value(f64::from(DEFAULT_TILE_SIZE));

        let shuffle = Button::new(ui, Rect::default(), "Shuffle")?
            .on_click(|| Some(Msg::Album(AlbumMsg::Shuffle)));
        let close = Button::new(ui, Rect::default(), "Close album")?
            .on_click(|| Some(Msg::Album(AlbumMsg::CloseAlbum)));

        Ok(Self {
            ui: ui.clone(),
            count: Label::new(ui, Rect::default(), "0 albums")?,
            sort_label: Label::new(ui, Rect::default(), "Sort")?,
            sort,
            size_label: Label::new(ui, Rect::default(), "Size")?,
            size,
            shuffle,
            close,
        })
    }

    /// Moves and sizes the toolbar inside `bounds` and returns the content area
    /// below it, where the grid and the track list go.
    pub(super) fn set_bounds(&self, bounds: Rect) -> Rect {
        let dpi = self.ui.dpi();
        let px = |value: f32| dip(value).to_px(dpi).value();
        let top = bounds.top;
        let band_right = bounds.right - px(INSET);
        let gap = px(GAP);
        let bottom = top + px(TOOLBAR_HEIGHT);

        let mut x = bounds.left + px(INSET);
        let count = Rect::new(x, top, x + px(COUNT_WIDTH), bottom);
        x += px(COUNT_WIDTH) + gap;
        let sort_label = Rect::new(x, top, x + px(SORT_LABEL_WIDTH), bottom);
        x += px(SORT_LABEL_WIDTH) + gap;
        let sort = Rect::new(x, top, x + px(SORT_WIDTH), bottom);
        x += px(SORT_WIDTH) + gap;
        let size_label = Rect::new(x, top, x + px(SIZE_LABEL_WIDTH), bottom);
        x += px(SIZE_LABEL_WIDTH) + gap;

        let close = Rect::new(band_right - px(CLOSE_WIDTH), top, band_right, bottom);
        let shuffle = Rect::new(
            close.left - gap - px(SHUFFLE_WIDTH),
            top,
            close.left - gap,
            bottom,
        );
        let size = Rect::new(x, top, shuffle.left - gap, bottom);

        self.ui.apply_moves(&[
            (self.count.id(), count),
            (self.sort_label.id(), sort_label),
            (self.sort.id(), sort),
            (self.size_label.id(), size_label),
            (self.size.id(), size),
            (self.shuffle.id(), shuffle),
            (self.close.id(), close),
        ]);

        Rect::new(bounds.left, bottom, bounds.right, bounds.bottom)
    }

    /// Shows the toolbar when `active`, and the Close button only when an album
    /// is also selected.
    pub(super) fn set_visible(&self, active: bool, selected: bool) {
        self.ui.set_visible(self.count.id(), active);
        self.ui.set_visible(self.sort_label.id(), active);
        self.ui.set_visible(self.sort.id(), active);
        self.ui.set_visible(self.size_label.id(), active);
        self.ui.set_visible(self.size.id(), active);
        self.ui.set_visible(self.shuffle.id(), active);
        self.ui.set_visible(self.close.id(), active && selected);
    }

    /// Mirrors the shared model's sort, size and selection onto the controls.
    pub(super) fn sync(&self, grid: &AlbumGrid) {
        self.count.set_text(&format!("{} albums", grid.len()));
        let index = sort_index(grid.sort);
        if self.sort.selected() != index {
            self.sort.select(index);
        }
        let size = f64::from(grid.tile_size);
        if (self.size.value() - size).abs() > f64::EPSILON {
            self.size.set_value(size);
        }
        self.shuffle.set_enabled(grid.selected_key().is_some());
    }
}

/// The sort combo's item labels, in [`AlbumSort::ALL`] order.
fn sort_labels() -> [&'static str; 4] {
    AlbumSort::ALL.map(|sort| sort.label())
}

/// The combo index of `sort`, or 0 when absent.
fn sort_index(sort: AlbumSort) -> usize {
    AlbumSort::ALL
        .iter()
        .position(|candidate| *candidate == sort)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_index_maps_every_sort_and_defaults_to_the_first() {
        for (index, sort) in AlbumSort::ALL.into_iter().enumerate() {
            assert_eq!(sort_index(sort), index);
            assert_eq!(sort_labels()[index], sort.label());
        }
    }
}
