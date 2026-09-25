#![forbid(unsafe_code)]

//! The owner-drawn projectM surface (#301): a [`CustomWidget`] that returns
//! [`Renderer::Gl`] and drives a runtime-loaded libprojectM instance through
//! [`EngineSlot`].
//!
//! libprojectM needs its OpenGL context current to be created, resized,
//! rendered and destroyed, so the engine is built lazily in the first
//! [`paint_gl`](CustomWidget::paint_gl) and freed in
//! [`gl_teardown`](CustomWidget::gl_teardown). When the libraries are missing
//! or no GL context can be created the widget switches to its GDI paint and
//! draws the [`fallback`](super::fallback) plasma instead.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::Instant;

use emusic_milkdrop::{Frame, MilkdropEngine, PlaceholderEngine};
use emusic_ui::player_api::PlayerApi;
use emusic_ui::state::projectm::{PresetRequest, ProjectMAvailability, ProjectMSettings};
use win32ui::gdi::{Canvas, Font};
use win32ui::glow;
use win32ui::{CustomWidget, Input, MouseButton, Rect, Renderer, Size, Theme, WidgetCx, dip};

use super::ProjectMEvent;
use super::ProjectMGesture;
use super::engine::EngineSlot;
use super::fallback;
use super::feed::{Feed, FramePacer};
use super::overlay;

/// The owner-drawn projectM surface.
pub(super) struct ProjectMWidget {
    /// Whether the surface is shown; a hidden surface does no work (#305).
    visible: Cell<bool>,
    /// Whether the pointer rests on the surface, so its overlay shows.
    hovered: Cell<bool>,
    /// The persisted settings pushed by the frontend.
    settings: RefCell<ProjectMSettings>,
    /// The preset to restore when the instance is created.
    last_preset: RefCell<Option<PathBuf>>,
    /// Preset navigation queued by the frontend, drained on the next paint.
    requests: RefCell<Vec<PresetRequest>>,
    /// Interleaved stereo PCM read from the player, waiting for a tick.
    feed: RefCell<Feed>,
    /// The FPS cap repaints are throttled to.
    pacer: RefCell<FramePacer>,
    /// projectM, once a GL context was available.
    engine: EngineSlot,
    /// The CPU placeholder driven while projectM is unavailable.
    placeholder: RefCell<PlaceholderEngine>,
    /// The placeholder's latest frame, painted by the GDI path.
    frame: Cell<Frame>,
    /// When the placeholder last advanced, for its `dt`.
    last_tick: Cell<Option<Instant>>,
    /// Events raised for the frontend to observe.
    events: RefCell<Vec<ProjectMEvent>>,
    /// The hint font, created once for the widget's DPI.
    font: Option<Font>,
    /// The widget's dots-per-inch.
    dpi: Cell<u32>,
}

impl ProjectMWidget {
    pub(super) fn new(dpi: u32) -> Self {
        let engine = EngineSlot::new();
        engine.load();
        Self {
            visible: Cell::new(true),
            hovered: Cell::new(false),
            settings: RefCell::new(ProjectMSettings::default()),
            last_preset: RefCell::new(None),
            requests: RefCell::new(Vec::new()),
            feed: RefCell::new(Feed::default()),
            pacer: RefCell::new(FramePacer::new(ProjectMSettings::default().fps_cap)),
            engine,
            placeholder: RefCell::new(PlaceholderEngine::new()),
            frame: Cell::new(Frame {
                hue: 0.0,
                pulse: 0.0,
                swirl: 0.0,
            }),
            last_tick: Cell::new(None),
            events: RefCell::new(Vec::new()),
            font: Font::system_ui(dpi).ok(),
            dpi: Cell::new(dpi),
        }
    }

    /// Shows or hides the surface.
    pub(super) fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        if !visible {
            self.hovered.set(false);
        }
    }

    /// Reads the player for this frame, buffering its samples (or silence).
    pub(super) fn feed(&self, player: &dyn PlayerApi) {
        self.feed.borrow_mut().feed(player);
    }

    /// Replaces the engine and preset settings.
    pub(super) fn set_settings(&self, settings: &ProjectMSettings) {
        let sanitized = settings.sanitized();
        if *self.settings.borrow() == sanitized {
            return;
        }
        self.pacer.borrow_mut().set_fps(sanitized.fps_cap);
        *self.settings.borrow_mut() = sanitized;
    }

    /// Sets the preset to restore the next time an instance is created.
    pub(super) fn set_last_preset(&self, path: Option<&Path>) {
        if self.last_preset.borrow().as_deref() == path {
            return;
        }
        *self.last_preset.borrow_mut() = path.map(Path::to_path_buf);
    }

    /// Queues a preset navigation request for the next paint.
    pub(super) fn request_preset(&self, request: PresetRequest) {
        self.requests.borrow_mut().push(request);
    }

    /// Takes the events raised since the last call, oldest first.
    pub(super) fn take_events(&self) -> Vec<ProjectMEvent> {
        std::mem::take(&mut *self.events.borrow_mut())
    }

    /// The engine status as last decided.
    pub(super) fn availability(&self) -> ProjectMAvailability {
        self.engine.availability()
    }

    /// Renders one frame with the context current, forwarding any engine
    /// events to the queue.
    fn render(&self, settings: &ProjectMSettings) {
        let mut requests = std::mem::take(&mut *self.requests.borrow_mut());
        let mut events = Vec::new();
        self.engine.render(settings, &mut requests, &mut events);
        if !events.is_empty() {
            self.events.borrow_mut().extend(events);
        }
    }

    /// Drains the buffered audio into the engine and advances the placeholder
    /// one frame.
    fn advance(&self) {
        let samples = self.feed.borrow_mut().take();
        let now = Instant::now();
        let dt = self
            .last_tick
            .replace(Some(now))
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());

        {
            let mut placeholder = self.placeholder.borrow_mut();
            placeholder.feed_pcm(&samples);
            self.frame.set(placeholder.tick(dt));
        }
        self.engine.add_pcm(&samples);
    }

    /// Queues a status event raised by the engine.
    fn queue_event(&self, event: Option<ProjectMEvent>) {
        if let Some(event) = event {
            self.events.borrow_mut().push(event);
        }
    }

    /// Tracks the pointer entering or leaving the surface, repainting the GDI
    /// fallback so its overlay appears or disappears.
    fn set_hovered(&self, cx: &mut WidgetCx<ProjectMGesture>, hovered: bool) {
        if self.hovered.replace(hovered) == hovered {
            return;
        }
        if self.engine.is_fallback() {
            cx.invalidate();
        }
    }
}

impl CustomWidget for ProjectMWidget {
    type Event = ProjectMGesture;

    fn preferred_size(&self, dpi: u32) -> Option<Size> {
        self.dpi.set(dpi);
        Some(Size::new(
            dip(320.0).to_px(dpi).value(),
            dip(240.0).to_px(dpi).value(),
        ))
    }

    fn renderer(&self) -> Renderer {
        if self.engine.is_fallback() {
            Renderer::Gdi
        } else {
            Renderer::Gl
        }
    }

    fn paint(&self, canvas: &Canvas, bounds: Rect, theme: &Theme) {
        let hint = fallback::hint(&self.engine.availability());
        fallback::paint(
            canvas,
            bounds,
            self.frame.get(),
            theme,
            self.font.as_ref(),
            hint,
        );
        if self.hovered.get() {
            let buttons = overlay::buttons(bounds, self.dpi.get());
            overlay::draw(canvas, &buttons, theme, self.font.as_ref(), self.dpi.get());
        }
    }

    fn paint_gl(&self, _gl: &glow::Context, bounds: Rect, _theme: &Theme) {
        self.engine.mark_gl_painted();
        let width = bounds.width().max(1) as usize;
        let height = bounds.height().max(1) as usize;
        let settings = self.settings.borrow();
        let last_preset = self.last_preset.borrow().clone();
        self.queue_event(
            self.engine
                .prepare(width, height, &settings, last_preset.as_deref()),
        );
        if self.engine.has_instance() {
            self.render(&settings);
        }
    }

    fn gl_teardown(&self, _gl: &glow::Context) {
        self.engine.teardown();
    }

    fn input(&self, input: Input, cx: &mut WidgetCx<ProjectMGesture>) {
        match input {
            Input::Frame => {
                let visible = self.visible.get();
                cx.request_animation(visible);
                if !visible {
                    return;
                }
                // The GL surface could not be created, so `paint_gl` never ran:
                // report it and let the next paint use the GDI fallback.
                self.queue_event(self.engine.report_no_gl());
                if self.engine.is_fallback() && self.pacer.borrow_mut().due(Instant::now()) {
                    cx.invalidate();
                }
            }
            Input::Tick => {
                if !self.visible.get() {
                    cx.request_animation(false);
                    return;
                }
                self.advance();
                if self.pacer.borrow_mut().due(Instant::now()) {
                    cx.invalidate();
                }
            }
            Input::MouseMove { .. } => self.set_hovered(cx, true),
            Input::MouseLeave => self.set_hovered(cx, false),
            Input::MouseDoubleClick {
                button: MouseButton::Left,
                ..
            } => cx.emit(ProjectMGesture::DoubleClick),
            Input::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } if self.engine.is_fallback() && self.hovered.get() => {
                // The buttons are only drawn on the GDI fallback; an invisible
                // GL overlay must not swallow clicks (see `overlay`).
                let buttons = overlay::buttons(cx.bounds(), cx.dpi());
                if let Some(action) = overlay::hit(&buttons, x, y) {
                    cx.emit(match action {
                        overlay::Action::PopOut => ProjectMGesture::PopOut,
                        overlay::Action::Fullscreen => ProjectMGesture::Fullscreen,
                        overlay::Action::Hide => ProjectMGesture::Hide,
                    });
                }
            }
            _ => {}
        }
    }
}
