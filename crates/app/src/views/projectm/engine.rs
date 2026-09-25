#![forbid(unsafe_code)]

//! Loading and driving the libprojectM instance behind the surface (#301).
//!
//! The instance is bound to the OpenGL context it was created on, so
//! [`EngineSlot`] is only ever advanced from [`paint_gl`](super::widget)
//! (which runs with the context current) and freed from its GL teardown hook.
//! It owns the status and version the frontend reports as
//! [`ProjectMAvailability`], so the widget can stay a thin paint/input shell.

use std::cell::{Cell, RefCell};
use std::path::Path;

use emusic_projectm::{Event as ProjectMFMTEvent, Instance, ProjectM, ProjectMError};
use emusic_ui::state::projectm::{PresetRequest, ProjectMAvailability, ProjectMSettings};

use super::ProjectMEvent;
use super::feed::parameters_from;

/// How the engine currently runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    /// No paint has decided yet.
    Unknown,
    /// projectM is loaded and rendering.
    Running,
    /// The projectM libraries could not be loaded.
    MissingLibrary,
    /// No OpenGL 3.3 context could be created.
    NoOpenGl,
}

/// A running projectM instance, bound to its GL context.
struct Engine {
    instance: Instance,
    /// The framebuffer size the instance was last resized to.
    size: (usize, usize),
}

/// The engine's slot in the widget: ownership, status and last reported
/// version.
pub(super) struct EngineSlot {
    engine: RefCell<Option<Engine>>,
    /// The loaded libraries, kept so the instance can be recreated on another
    /// context. Loaded up front so a missing library never creates a GL
    /// surface (which would leave the window unpaintable with GDI).
    libs: RefCell<Option<ProjectM>>,
    status: Cell<Status>,
    version: RefCell<Option<String>>,
    /// The settings last pushed into the instance.
    settings: RefCell<ProjectMSettings>,
    /// Set once a paint reached the GL path, so a frame that never did can be
    /// reported as [`Status::NoOpenGl`].
    gl_painted: Cell<bool>,
}

impl EngineSlot {
    pub(super) fn new() -> Self {
        Self {
            engine: RefCell::new(None),
            libs: RefCell::new(None),
            status: Cell::new(Status::Unknown),
            version: RefCell::new(None),
            settings: RefCell::new(ProjectMSettings::default()),
            gl_painted: Cell::new(false),
        }
    }

    /// Loads the projectM libraries. Called once when the widget is built, so
    /// the first paint can pick GDI straight away when they are missing.
    pub(super) fn load(&self) {
        match ProjectM::load() {
            Ok(projectm) => *self.libs.borrow_mut() = Some(projectm),
            Err(err) => {
                tracing::warn!(%err, "projectM libraries are not installed; drawing the placeholder");
                self.status.set(Status::MissingLibrary);
            }
        }
    }

    /// Whether projectM could not run, so the placeholder is painted.
    pub(super) fn is_fallback(&self) -> bool {
        matches!(self.status.get(), Status::MissingLibrary | Status::NoOpenGl)
    }

    /// Whether an instance is alive and ready to render.
    pub(super) fn has_instance(&self) -> bool {
        self.engine.borrow().is_some()
    }

    /// The engine status as last decided.
    pub(super) fn availability(&self) -> ProjectMAvailability {
        match self.status.get() {
            Status::Unknown => ProjectMAvailability::Unknown,
            Status::Running => {
                ProjectMAvailability::Available(self.version.borrow().clone().unwrap_or_default())
            }
            Status::MissingLibrary => ProjectMAvailability::MissingLibrary,
            Status::NoOpenGl => ProjectMAvailability::NoOpenGl,
        }
    }

    /// Notes that a frame reached the GL paint.
    pub(super) fn mark_gl_painted(&self) {
        self.gl_painted.set(true);
    }

    /// Reports that the GL surface could not be created, if the first paint
    /// never reached [`CustomWidget::paint_gl`](win32ui::CustomWidget::paint_gl).
    pub(super) fn report_no_gl(&self) -> Option<ProjectMEvent> {
        if self.status.get() == Status::Unknown && !self.gl_painted.get() {
            self.change(Status::NoOpenGl)
        } else {
            None
        }
    }

    /// Creates the instance on the first paint, resizing it as the surface
    /// changes. Any status change is returned as an event for the frontend.
    pub(super) fn prepare(
        &self,
        width: usize,
        height: usize,
        settings: &ProjectMSettings,
        last_preset: Option<&Path>,
    ) -> Option<ProjectMEvent> {
        {
            let mut engine = self.engine.borrow_mut();
            if let Some(engine) = engine.as_mut() {
                if engine.size != (width, height) {
                    if let Err(err) = engine.instance.resize(width, height) {
                        tracing::warn!(%err, "projectM resize failed");
                        return None;
                    }
                    engine.size = (width, height);
                }
                return None;
            }
        }

        if self.is_fallback() {
            return None;
        }

        let Some(projectm) = self.libs.borrow().as_ref().cloned() else {
            return self.change(Status::MissingLibrary);
        };
        let instance = match projectm.create(width, height) {
            Ok(instance) => instance,
            Err(err) => {
                tracing::warn!(%err, "could not create a projectM instance; drawing the placeholder");
                return self.change(status_for_create_error(&err));
            }
        };

        *self.version.borrow_mut() = Some(projectm.version());
        instance.set_parameters(&parameters_from(settings));
        *self.settings.borrow_mut() = settings.clone();
        if let Some(path) = last_preset
            && path.is_file()
            && let Err(err) = instance.load_preset(path, false)
        {
            tracing::warn!(%err, path = %path.display(), "could not restore the last preset");
        }

        *self.engine.borrow_mut() = Some(Engine {
            instance,
            size: (width, height),
        });
        self.change(Status::Running)
    }

    /// Renders one frame, applying changed settings and queued preset requests
    /// first, and appends the events the playlist raised to `events`.
    pub(super) fn render(
        &self,
        settings: &ProjectMSettings,
        requests: &mut Vec<PresetRequest>,
        events: &mut Vec<ProjectMEvent>,
    ) {
        let engine = self.engine.borrow();
        let Some(engine) = engine.as_ref() else {
            return;
        };

        if *self.settings.borrow() != *settings {
            engine.instance.set_parameters(&parameters_from(settings));
            *self.settings.borrow_mut() = settings.clone();
        }

        let hard_cut = settings.hard_cuts;
        for request in requests.drain(..) {
            if let Err(err) = apply_request(engine, request, hard_cut) {
                tracing::warn!(%err, "projectM preset request failed");
            }
        }

        if let Err(err) = engine.instance.render() {
            tracing::warn!(%err, "projectM render failed");
        }
        for event in engine.instance.take_events() {
            match event {
                ProjectMFMTEvent::PresetSwitched { index, .. } => {
                    if let Some(path) = engine.instance.preset_path(index) {
                        events.push(ProjectMEvent::PresetShown(path));
                    }
                }
                ProjectMFMTEvent::PresetFailed { file, message } => {
                    tracing::warn!(file, message, "projectM skipped a preset");
                }
            }
        }
    }

    /// Feeds interleaved stereo samples to the running instance.
    pub(super) fn add_pcm(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        if let Some(engine) = self.engine.borrow().as_ref() {
            engine.instance.add_pcm(samples);
        }
    }

    /// Frees the instance and its GPU resources; its context must be current.
    pub(super) fn teardown(&self) {
        self.gl_painted.set(false);
        if let Some(engine) = self.engine.borrow_mut().take()
            && let Err((_instance, err)) = engine.instance.destroy()
        {
            tracing::warn!(%err, "could not destroy the projectM instance");
        }
    }

    /// Records a status change and returns the matching event.
    fn change(&self, status: Status) -> Option<ProjectMEvent> {
        if self.status.get() == status {
            return None;
        }
        self.status.set(status);
        Some(ProjectMEvent::AvailabilityChanged(self.availability()))
    }
}

/// Maps a failed instance creation onto an availability status: the
/// context-bound errors mean GL is unusable, anything else means the library.
fn status_for_create_error(err: &ProjectMError) -> Status {
    match err {
        ProjectMError::DllNotFound(_) | ProjectMError::MissingSymbol(_) => Status::MissingLibrary,
        ProjectMError::NoCurrentContext
        | ProjectMError::WrongContext
        | ProjectMError::GlewInit(_)
        | ProjectMError::CreateFailed(_)
        | ProjectMError::InvalidString(_) => Status::NoOpenGl,
    }
}

/// Applies one queued preset request via `hard_cut`.
fn apply_request(
    engine: &Engine,
    request: PresetRequest,
    hard_cut: bool,
) -> std::result::Result<(), ProjectMError> {
    match request {
        PresetRequest::Next => engine.instance.play_next(hard_cut).map(|_| ()),
        PresetRequest::Previous => engine.instance.play_previous(hard_cut).map(|_| ()),
        PresetRequest::Random => {
            let count = engine.instance.preset_count();
            if count == 0 {
                return Ok(());
            }
            engine
                .instance
                .play_index(random_index(count), hard_cut)
                .map(|_| ())
        }
    }
}

/// A cheap pseudo-random playlist index, seeded by the wall clock.
fn random_index(count: usize) -> usize {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    nanos as usize % count
}
