//! Lays out the shell bands and the active central view from the client rect.

use emusic_ui::state::View;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;

use super::Win32App;
use crate::menu;
use crate::window::CAPTION_HEIGHT;

/// The transport band's height, in device-independent pixels.
const TOP_BAR_HEIGHT: f32 = 40.0;
/// The bottom status band's height, in device-independent pixels.
const STATUS_BAR_HEIGHT: f32 = 24.0;

impl Win32App {
    /// Positions the caption band, the transport band, the status bar and the
    /// central area from the client rectangle.
    pub(super) fn relayout(&mut self) {
        let client = self.ui.client_rect();
        let dpi = self.ui.dpi();
        let caption = self.caption_inset_px(dpi);
        let menu_h = dip(menu::MENU_BAR_HEIGHT).to_px(dpi).value();
        let top_bar = dip(TOP_BAR_HEIGHT).to_px(dpi).value();
        let bar_top = client.top + caption;
        if let Some(id) = self.menu.id() {
            self.ui.apply_moves(&[(
                id,
                Rect::new(client.left, bar_top, client.right, bar_top + menu_h),
            )]);
        }
        let band_top = bar_top + menu_h;
        let bar_bottom = band_top + top_bar;
        self.top_bar.set_bounds(
            Rect::new(client.left, client.top, client.right, bar_top),
            Rect::new(client.left, band_top, client.right, bar_bottom),
        );

        let status = if self.shell.state.panels.status_bar {
            dip(STATUS_BAR_HEIGHT).to_px(dpi).value()
        } else {
            0
        };
        let bottom = client.bottom - status;
        self.status_bar
            .set_bounds(Rect::new(client.left, bottom, client.right, client.bottom));

        let top = bar_bottom;
        let navigator_width = if self.shell.state.panels.navigator {
            dip(self.shell.state.navigator_width).to_px(dpi).value()
        } else {
            0
        };
        self.navigator.set_bounds(Rect::new(
            client.left,
            top,
            client.left + navigator_width,
            bottom,
        ));
        // The right panel is a fixed-width strip on the trailing edge, beside
        // every central view while `panels.right_panel` is on. Its saved width
        // is clamped so the central area never disappears.
        let central_left = client.left + navigator_width;
        let right_width = if self.shell.state.panels.right_panel {
            dip(self.shell.state.right_panel_width)
                .to_px(dpi)
                .value()
                .min((client.right - central_left).max(0))
        } else {
            0
        };
        let central_right = client.right - right_width;
        self.right_panel
            .set_bounds(Rect::new(central_right, top, client.right, bottom));
        let central = Rect::new(central_left, top, central_right, bottom);
        self.central_bounds = central;
        match self.shell.state.view {
            View::Music => {
                // The browser strip sits above the header + table; hidden, it
                // takes no space.
                let browser_height = if self.shell.state.music.browser.visible {
                    dip(self.shell.state.music.browser.height)
                        .to_px(dpi)
                        .value()
                        .min(bottom - top)
                } else {
                    0
                };
                self.browser.set_bounds(Rect::new(
                    central.left,
                    central.top,
                    central.right,
                    central.top + browser_height,
                ));
                self.music.set_bounds(Rect::new(
                    central.left,
                    central.top + browser_height,
                    central.right,
                    central.bottom,
                ));
            }
            View::Folders => self.folders.set_bounds(central),
            View::Albums => self.albums.set_bounds(central),
            View::Artists => self.artists.set_bounds(central),
            View::Genres => self.genres.set_bounds(central),
            View::Starred => self.starred.set_bounds(central),
            View::MostPlayed => self.most_played.set_bounds(central),
            View::History => self.history.set_bounds(central),
            View::Settings => self.settings.set_bounds(central),
            View::NowPlaying => self.now_playing.set_bounds(central),
            View::Visualization => self.visualization.set_bounds(central),
        }
    }

    /// The caption band height in device pixels: the backend's reserved caption
    /// inset when the chrome is attached, else the requested constant.
    fn caption_inset_px(&self, dpi: u32) -> i32 {
        self.chrome.as_ref().map_or_else(
            || dip(CAPTION_HEIGHT).to_px(dpi).value(),
            |chrome| chrome.caption_inset().to_px(dpi).value(),
        )
    }
}
