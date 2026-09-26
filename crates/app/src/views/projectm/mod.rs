#![forbid(unsafe_code)]

//! The Win32 projectM visualization surface (#301).
//!
//! [`ProjectMView`] is the one owner-drawn widget every projectM surface
//! hosts — the panel (#302), the independent window (#303) and fullscreen
//! (#304). It renders with OpenGL through `xui`'s [`Renderer::Gl`] and
//! drives a runtime-loaded libprojectM instance ([`emusic_projectm`]),
//! falling back to a CPU plasma ([`emusic_milkdrop`]) plus a one-line hint
//! when the libraries are missing or no OpenGL context can be created.
//!
//! The engine is bound to the GL context it was created on, so the frontend
//! treats the surface as three independent things: the widget (this module),
//! the [`ProjectMAvailability`] it last reported, and the [`ProjectMEvent`]s
//! it raises (a preset started showing, the availability changed). The view
//! keeps no reference to the shell; the frontend pushes settings and audio
//! into it and drains its events, so #302 can wire it without back-references.
//!
//! [`Renderer::Gl`]: xui::Renderer::Gl

mod engine;
mod fallback;
mod feed;
mod grace;
mod overlay;
mod presets;
mod widget;
mod window;

pub(crate) use grace::GraceTimer;
pub(crate) use presets::{PresetFiles, PresetRoots, PresetScanner};
pub(crate) use window::VizWindow;

use std::path::{Path, PathBuf};

use emusic_ui::player_api::PlayerApi;
use emusic_ui::state::projectm::{PresetRequest, ProjectMAvailability, ProjectMSettings};
use xui::prelude::*;

use crate::app::Msg;

use widget::ProjectMWidget;

/// Something the surface did that the frontend may want to mirror into the
/// shared [`ProjectMState`](emusic_ui::state::projectm::ProjectMState).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectMEvent {
    /// The playlist moved to `index`, showing its file.
    PresetShown(PathBuf),
    /// The engine status changed.
    AvailabilityChanged(ProjectMAvailability),
}

/// An input gesture on a projectM surface the frontend turns into a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectMGesture {
    /// The surface was double-clicked.
    DoubleClick,
    /// The "pop out" overlay button.
    PopOut,
    /// The fullscreen overlay button (or a double-click).
    Fullscreen,
    /// The hide overlay button.
    Hide,
    /// The surface was right-clicked; show its context menu (#306).
    ContextMenu,
}

/// The projectM surface: a GL custom widget plus its feed, settings and preset
/// requests. Host it like any other control and drive it from the shell tick.
///
/// Generic over the host window's message type `M` so the panel can use the
/// app's [`Msg`] and the independent window (#303) can use its own, forwarding
/// gestures back through a [`Proxy`](xui::Proxy).
pub struct ProjectMView<M = Msg> {
    custom: Custom<ProjectMWidget, M>,
}

impl<M: 'static> ProjectMView<M> {
    /// Creates the surface. Its widget paints a placeholder until projectM can
    /// start, so construction never needs the DLLs or a GL context.
    pub fn new(ui: &mut Ui<M>) -> xui::Result<Self> {
        let widget = ProjectMWidget::new(ui.dpi());
        Ok(Self {
            custom: Custom::new(ui, widget)?,
        })
    }

    /// Maps the surface's hover/double-click [`ProjectMGesture`]s to a host
    /// message. Without it the gestures are dropped.
    #[must_use]
    pub fn with_gestures(self, map: impl Fn(ProjectMGesture) -> Option<M> + 'static) -> Self {
        Self {
            custom: self.custom.on_event(map),
        }
    }

    /// Shows or hides the surface. While hidden it stops: no repaints, no
    /// audio read, no GPU work (#305).
    pub fn set_visible(&self, visible: bool) {
        self.custom.set_visible(visible);
        self.custom.widget().borrow().set_visible(visible);
        self.custom.invalidate();
    }

    /// Starts or stops the surface's animation ticks without hiding its
    /// window, so a minimised app stops burning frames but keeps its place
    /// (#305).
    pub fn set_running(&self, running: bool) {
        self.custom.widget().borrow().set_visible(running);
        if running {
            self.custom.invalidate();
        }
    }

    /// Frees the projectM instance through the GL hook, with its context
    /// current, so video memory is released without waiting for a paint (#305).
    pub fn suspend(&self) {
        let widget = self.custom.widget();
        let _ = self.custom.with_gl(|_| widget.borrow().suspend());
    }

    /// Replaces the scanned preset file list and texture folders.
    pub(crate) fn set_presets(&self, files: PresetFiles) {
        self.custom.widget().borrow().set_presets(files);
    }

    /// Reads the player for this frame, feeding samples while it plays and
    /// silence while it is paused or stopped.
    pub fn feed(&self, player: &dyn PlayerApi) {
        self.custom.widget().borrow().feed(player);
    }

    /// Buffers raw interleaved stereo samples. Used by the independent window
    /// (#303), which the main app feeds without handing it a player.
    pub fn feed_samples(&self, samples: &[f32]) {
        self.custom.widget().borrow().feed_samples(samples);
    }

    /// Buffers a block of silence, so the visualization keeps moving while the
    /// player is paused or stopped.
    pub fn push_silence(&self) {
        self.custom.widget().borrow().push_silence();
    }

    /// Replaces the engine and preset settings, applied to a running instance.
    pub fn set_settings(&self, settings: &ProjectMSettings) {
        self.custom.widget().borrow().set_settings(settings);
    }

    /// Sets the preset to restore the next time an instance is created.
    pub fn set_last_preset(&self, path: Option<&Path>) {
        self.custom.widget().borrow().set_last_preset(path);
    }

    /// Queues a preset navigation request for the running engine.
    pub fn request_preset(&self, request: PresetRequest) {
        self.custom.widget().borrow().request_preset(request);
    }

    /// Takes the events raised since the last call, oldest first.
    #[must_use]
    pub fn take_events(&self) -> Vec<ProjectMEvent> {
        self.custom.widget().borrow().take_events()
    }

    /// The engine status as last decided by a paint.
    #[must_use]
    pub fn availability(&self) -> ProjectMAvailability {
        self.custom.widget().borrow().availability()
    }
}

impl<M> AsControl for ProjectMView<M> {
    fn control(&self) -> &Control {
        self.custom.control()
    }
}
