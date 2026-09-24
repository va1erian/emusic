# Visualizer strip on the win32 frontend

What it takes to bring `VisualizerMode` (spectrum, oscilloscope, milkdrop —
see `crates/app/src/panels/visualizer/`) to `crates/win32`, and what, if
anything, needs to change in [`win32ui`](https://github.com/va1erian/win32ui)
first.

## Current state

`crates/win32/src/views/status_bar.rs` wraps win32ui's `StatusBar`, a plain
native common control with three text parts. It has no drawing surface at
all — there is nowhere to paint bars, a trace, or rings today.

The audio data side needs no work either way: `PlayerApi::fft()` and
`::samples()` (used by the egui spectrum/oscilloscope already) are
toolkit-agnostic and already live in `emusic-ui`, shared by both frontends.

## Spectrum and oscilloscope: no win32ui changes needed

win32ui already has the right extension point: `CustomWidget` +
`Custom<W, M>` (`src/controls/custom/`). Implementing `CustomWidget` gives a
child `HWND` hosted like any other control, a `paint`/`paint_d2d` callback,
typed `Input` (mouse/keyboard/focus), and — importantly —
`WidgetCx::request_animation` plus an `Input::Tick`, i.e. exactly the
`FRAME_INTERVAL`-driven repaint the egui strip already uses.

So porting `spectrum.rs`/`scope.rs` is a rewrite of the *drawing calls* only:

- egui's `Painter::rect_filled`/`Shape::line` → win32ui's GDI `Canvas` (or,
  for the anti-aliased look, `Renderer::Direct2D` + `paint_d2d`'s `D2dCanvas`
  — closer to what the egui version already looks like).
- Own a small widget struct (peak-hold `Vec<f32>`, engine state) exactly like
  `VisualizerState` does today, behind a `Cell`/`RefCell` since
  `CustomWidget` methods take `&self`.
- Wire it into `views/status_bar.rs` next to the existing `StatusBar` control
  (or replace the status bar's third part with it, matching the strip's
  position in the egui shell).

This is regular `crates/win32` application work — no upstream issue required.

## Milkdrop: needs a new win32ui capability

The current placeholder engine (`emusic-milkdrop::PlaceholderEngine`) is pure
math (`Frame { hue, pulse, swirl }`) and paints as plain shapes, so it is
*also* portable today with the GDI/Direct2D path above — no blocker there.

The blocker is a **real MilkDrop/projectM engine**, which renders with
OpenGL into whatever `HWND`/`HDC` it's given (`renderFrame()` issues GL draw
calls directly; see the investigation this crate's docs reference). win32ui
has exactly two paint backends today, `Renderer::Gdi` and `Renderer::Direct2D`
(`src/controls/custom/widget.rs`), both of which own the surface themselves
(`RendererState` in `src/controls/custom/d2d.rs` lazily creates a
`D2dSurface` from the widget's `Hwnd` on first paint, resizes it on
`WM_SIZE`, and recreates it after device loss). There is no path today for a
widget to get a raw `HDC`/`HWND` to attach a WGL context to.

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

None of this needs a new windowing/event-loop dependency (see the "SDL vs.
raw WGL" discussion this doc's history follows) — it's the same shape as the
existing Direct2D integration, just targeting WGL instead of Direct2D's
device/swap chain, and it keeps win32ui's own `unsafe` isolated to `sys/`
exactly like every other backend.

### Scope

This is upstream work in `win32ui`, not `emusic`: emusic only consumes
`CustomWidget`. Filing it as a win32ui issue (`Renderer::Gl` for
`CustomWidget`) is the right next step before attempting a win32 Milkdrop
panel; the egui-side placeholder/spectrum/oscilloscope panels don't need to
wait on it.
