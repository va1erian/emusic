# Visualizer strip

The top-bar visualizer strip renders `VisualizerMode` (spectrum, oscilloscope,
and — later — Milkdrop/projectM). The spectrum and oscilloscope are implemented
in `crates/app/src/views/visualizer.rs` as a `win32ui` `CustomWidget` hosted by
the top bar, repainted on the shared
`emusic_ui::panels::visualizer::FRAME_INTERVAL`.

## Audio data

`PlayerApi::fft()` and `::samples()` are toolkit-agnostic and live in
`emusic-ui`. The widget owns its peak-hold/engine state behind a
`Cell`/`RefCell`, because `CustomWidget` methods take `&self`, and paints with
GDI's `Canvas` or `paint_d2d`'s `D2dCanvas` for the anti-aliased look.

## Milkdrop / projectM: needs a new win32ui capability

The placeholder engine (`emusic-milkdrop::PlaceholderEngine`) is pure math
(`Frame { hue, pulse, swirl }`) and paints as plain shapes, so it runs on the
GDI/Direct2D path above. A **real** MilkDrop/projectM engine renders with
OpenGL into whatever `HWND`/`HDC` it is given. win32ui has exactly two paint
backends today, `Renderer::Gdi` and `Renderer::Direct2D`
(`src/controls/custom/widget.rs`), both of which own the surface themselves
(`RendererState` in `src/controls/custom/d2d.rs` lazily creates a `D2dSurface`
from the widget's `Hwnd` on first paint, resizes it on `WM_SIZE`, and recreates
it after device loss). There is no path today for a widget to get a raw
`HDC`/`HWND` to attach a WGL context to.

### What win32ui would need to add

Mirroring the existing Direct2D path (`RendererState` / `D2dSurface`) is the
natural shape for a third renderer:

1. **`Renderer::Gl`** added to the `Renderer` enum (`src/controls/custom/widget.rs`).
2. **A `GlSurface`** (`src/gl/` or `src/controls/custom/gl.rs`, unsafe
   isolated in `src/sys/gl/` the way `src/sys/d2d/` isolates Direct2D's),
   created lazily on first paint from the widget's `Hwnd`:
   - Classic WGL bring-up: set a pixel format on the child's `HDC`
     (`SetPixelFormat`), create a legacy context just to load
     `wglCreateContextAttribsARB`/`wglChoosePixelFormatARB`, then create the
     real (core-profile) context and discard the legacy one.
   - `resize(width, height)` → `glViewport`, called from `WM_SIZE` exactly
     like `RendererState::resize` does for Direct2D today.
   - Falls back the same way Direct2D does (`RendererState::Gdi`) if context
     creation fails, so a widget author doesn't need to special-case old
     drivers.
3. **`CustomWidget::paint_gl(&self, gl: &glow::Context, bounds, theme)`**
   (default no-op, like `paint_d2d`), called with the context current and
   the framebuffer already sized/cleared; the widget issues its own GL calls
   (or, for a real projectM binding, calls into its FFI with the context
   current) and win32ui handles `SwapBuffers`.
4. Swap interval / vsync (`WGL_EXT_swap_control`) and multi-monitor DPI
   change handling need the same care the Direct2D path already gives its
   surface.

None of this needs a new windowing/event-loop dependency — it is the same
shape as the existing Direct2D integration, just targeting WGL instead of
Direct2D's device/swap chain, and it keeps win32ui's own `unsafe` isolated to
`sys/` exactly like every other backend.

### Scope

This is upstream work in `win32ui`, not `emusic`: emusic only consumes
`CustomWidget`. Filing it as a win32ui issue (`Renderer::Gl` for
`CustomWidget`) is the right next step before attempting an app-side Milkdrop
or projectM panel (see #295).
