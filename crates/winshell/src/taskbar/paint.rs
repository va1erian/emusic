//! GDI rendering of the iconic taskbar thumbnail (#322).
//!
//! The panel is drawn into a top-down 32bpp DIB section, the form
//! [`DwmSetIconicThumbnail`](windows::Win32::Graphics::Dwm::DwmSetIconicThumbnail)
//! requires: background and cover art are composed in the pixel buffer, then
//! GDI rasterises the text over them. GDI clears the alpha byte of the glyph
//! pixels (it treats the DIB as `X8R8G8B8`), so the alpha channel is forced
//! back to opaque afterwards — DWM reads it and would otherwise drop the text.
//!
//! This is one of the crate's raw Win32 surfaces: every `unsafe` block carries
//! a `// SAFETY:` comment.

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::mem::size_of;

#[cfg(windows)]
use windows::Win32::Foundation::{COLORREF, HANDLE, RECT};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLEARTYPE_QUALITY, CreateCompatibleDC, CreateDIBSection,
    CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE,
    DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FF_DONTCARE, HALFTONE, HBITMAP, HDC, HFONT,
    SelectObject, SetBkMode, SetStretchBltMode, SetTextColor, StretchDIBits, TRANSPARENT,
};

#[cfg(windows)]
use crate::taskbar_list::to_io;
use crate::{Result, WinshellError};

use super::layout::{self, Panel, PanelLayout, ThumbnailSize};

#[cfg(windows)]
const BACKGROUND: u32 = 0xFF20_2024;
#[cfg(windows)]
const TITLE_COLOR: COLORREF = COLORREF(0x00F0_F0F0);
#[cfg(windows)]
const MUTED_COLOR: COLORREF = COLORREF(0x00B4_B4B4);
#[cfg(windows)]
const TIME_COLOR: COLORREF = COLORREF(0x00DC_DCDC);

#[cfg(windows)]
pub(crate) fn render(size: ThumbnailSize, panel: &Panel<'_>) -> Result<HBITMAP> {
    // DWM's requested maximum is trusted only up to a sane bound so a bogus
    // `lParam` cannot allocate an enormous bitmap.
    let width = size.width.clamp(1, 4096) as i32;
    let height = size.height.clamp(1, 4096) as i32;
    let layout = layout::layout(ThumbnailSize {
        width: width as u32,
        height: height as u32,
    });

    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    // Negative height makes the DIB top-down, matching the row order DWM and
    // the cover buffer use.
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB.0;

    let mut bits: *mut c_void = std::ptr::null_mut();
    // SAFETY: `info` is a fully initialised `BITMAPINFO`; `bits` is a valid
    // out-pointer. A null source DC with `BI_RGB` is the documented way to ask
    // for an uninitialised DIB section, whose returned handle we own.
    let bitmap = match unsafe {
        CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0)
    } {
        Ok(bitmap) => bitmap,
        Err(err) => return Err(to_io(err)),
    };
    if bits.is_null() {
        // SAFETY: `bitmap` was just created by `CreateDIBSection` and is not
        // used after this branch.
        unsafe {
            let _ = DeleteObject(bitmap);
        }
        return Err(WinshellError::Io(std::io::Error::other(
            "CreateDIBSection returned no pixels",
        )));
    }

    // SAFETY: `bitmap` is a live 32bpp top-down DIB section of `width` x
    // `height` pixels, so `bits` points at exactly `width * height * 4`
    // writable bytes for this call.
    let pixels = unsafe {
        std::slice::from_raw_parts_mut(bits as *mut u8, width as usize * height as usize * 4)
    };
    fill_background(pixels, BACKGROUND);

    // SAFETY: a null source DC is the documented way to create a memory DC
    // compatible with the (non-existent) display; the returned DC is owned by
    // us and destroyed below.
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.0.is_null() {
        // SAFETY: `bitmap` was created above and is not used again here.
        unsafe {
            let _ = DeleteObject(bitmap);
        }
        return Err(WinshellError::Io(std::io::Error::other(
            "CreateCompatibleDC failed",
        )));
    }

    // SAFETY: `dc` and `bitmap` are live; `old` is the previously selected
    // object, restored below so both can be destroyed cleanly.
    let old = unsafe { SelectObject(dc, bitmap) };
    draw_panel(dc, &layout, panel);
    force_opaque(pixels);
    // SAFETY: `old` came from selecting `bitmap` into `dc`; restoring it lets
    // the DC and the bitmap be released without a dangling selection.
    unsafe {
        SelectObject(dc, old);
        let _ = DeleteDC(dc);
    }
    Ok(bitmap)
}

#[cfg(windows)]
fn fill_background(pixels: &mut [u8], color: u32) {
    let bytes = color.to_le_bytes();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&bytes);
    }
}

#[cfg(windows)]
fn force_opaque(pixels: &mut [u8]) {
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel[3] = 0xFF;
    }
}

#[cfg(windows)]
fn draw_panel(dc: HDC, layout: &PanelLayout, panel: &Panel<'_>) {
    draw_cover(dc, layout, panel.cover.as_ref());
    let title = text_font(layout.title_px, 600);
    let body = text_font(layout.body_px, 400);
    draw_line(dc, title, layout.title, panel.title, TITLE_COLOR);
    draw_line(dc, body, layout.artist, panel.artist, MUTED_COLOR);
    draw_line(dc, body, layout.album, panel.album, MUTED_COLOR);
    let time = layout::time_text(panel.elapsed_secs, panel.total_secs);
    draw_line(dc, body, layout.time, &time, TIME_COLOR);
    for font in [title, body] {
        if !font.0.is_null() {
            unsafe {
                let _ = DeleteObject(font);
            }
        }
    }
}

#[cfg(windows)]
fn draw_cover(dc: HDC, layout: &PanelLayout, cover: Option<&layout::Cover<'_>>) {
    let Some(cover) = cover else {
        return;
    };
    let target = layout.cover;
    if target.is_empty()
        || cover.width == 0
        || cover.height == 0
        || cover.rgba.len() < cover.width as usize * cover.height as usize * 4
    {
        return;
    }

    let mut bgra = Vec::with_capacity(cover.rgba.len());
    for pixel in cover.rgba.as_chunks::<4>().0 {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 0xFF]);
    }
    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = cover.width as i32;
    info.bmiHeader.biHeight = -(cover.height as i32);
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB.0;

    unsafe {
        SetStretchBltMode(dc, HALFTONE);
        StretchDIBits(
            dc,
            target.left,
            target.top,
            target.width(),
            target.height(),
            0,
            0,
            cover.width as i32,
            cover.height as i32,
            Some(bgra.as_ptr().cast()),
            &info,
            DIB_RGB_COLORS,
            windows::Win32::Graphics::Gdi::SRCCOPY,
        );
    }
}

#[cfg(windows)]
fn draw_line(dc: HDC, font: HFONT, rect: layout::Rect, text: &str, color: COLORREF) {
    if text.is_empty() || rect.is_empty() || font.0.is_null() {
        return;
    }
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut native = RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    };
    unsafe {
        let old = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, color);
        DrawTextW(
            dc,
            &mut wide,
            &mut native,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(dc, old);
    }
}

#[cfg(windows)]
fn text_font(pixels: i32, weight: i32) -> HFONT {
    let pixels = pixels.clamp(6, 256);
    unsafe {
        CreateFontW(
            -pixels,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            0,
            0,
            CLEARTYPE_QUALITY.0 as u32,
            FF_DONTCARE.0 as u32,
            windows::core::w!("Segoe UI"),
        )
    }
}

#[cfg(not(windows))]
pub(crate) fn render(_size: ThumbnailSize, _panel: &Panel<'_>) -> Result<()> {
    Err(WinshellError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Not supported on non-windows",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taskbar::layout::{Cover, Panel};
    use windows::Win32::Graphics::Gdi::GetDIBits;

    fn panel(cover: Option<&[u8]>) -> Panel<'_> {
        Panel {
            title: "A reasonably long title",
            artist: "The Artist",
            album: "The Album",
            elapsed_secs: 65,
            total_secs: 200,
            cover: cover.map(|rgba| Cover {
                width: 16,
                height: 16,
                rgba,
            }),
        }
    }

    /// A 16x16 opaque red square, row-major RGBA.
    fn cover_rgba() -> Vec<u8> {
        (0..16 * 16)
            .flat_map(|_| [0xC0, 0x40, 0x40, 0xFF])
            .collect()
    }

    #[test]
    fn render_draws_a_bitmap_with_and_without_a_cover() {
        let rgba = cover_rgba();
        for has_cover in [false, true] {
            let panel = panel(has_cover.then_some(rgba.as_slice()));
            let size = ThumbnailSize {
                width: 300,
                height: 150,
            };
            let bitmap = render(size, &panel).expect("render the panel");
            assert!(!bitmap.0.is_null(), "a DIB was created");
            // SAFETY: `bitmap` came from `render` and is not used again.
            unsafe {
                let _ = DeleteObject(bitmap);
            }
        }
    }

    #[test]
    fn render_keeps_every_pixel_opaque_for_dwm() {
        let rgba = cover_rgba();
        let panel = panel(Some(&rgba));
        let size = ThumbnailSize {
            width: 160,
            height: 90,
        };
        let bitmap = render(size, &panel).expect("render the panel");
        // SAFETY: a null source DC is the documented way to create a memory DC;
        // the returned DC is owned by this test and destroyed below.
        let dc = unsafe { CreateCompatibleDC(None) };
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = size.width as i32;
        info.bmiHeader.biHeight = -(size.height as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;
        let mut pixels = vec![0u8; size.width as usize * size.height as usize * 4];
        // SAFETY: `dc` and `bitmap` are live; `info` and `pixels` are valid for
        // the call and outlive it.
        let lines = unsafe {
            GetDIBits(
                dc,
                bitmap,
                0,
                size.height,
                Some(pixels.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        assert_eq!(lines, size.height as i32, "all scan lines were copied");
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 0xFF),
            "GDI clears the glyph alpha; render must restore it or DWM drops the text"
        );
        // SAFETY: both handles came from this test and are not used again.
        unsafe {
            let _ = DeleteDC(dc);
            let _ = DeleteObject(bitmap);
        }
    }

    #[test]
    fn render_of_a_degenerate_size_still_returns_a_bitmap() {
        let panel = panel(None);
        let bitmap = render(
            ThumbnailSize {
                width: 0,
                height: 0,
            },
            &panel,
        )
        .expect("render");
        assert!(!bitmap.0.is_null());
        // SAFETY: `bitmap` came from `render` and is not used again.
        unsafe {
            let _ = DeleteObject(bitmap);
        }
    }
}
