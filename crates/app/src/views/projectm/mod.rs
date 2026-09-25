#![forbid(unsafe_code)]

//! The Win32 projectM visualization surface (#301).
//!
//! [`ProjectMView`] is the one owner-drawn widget every projectM surface
//! hosts — the panel (#302), the independent window (#303) and fullscreen
//! (#304). It renders with OpenGL through `win32ui`'s [`Renderer::Gl`] and
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
//! [`Renderer::Gl`]: win32ui::Renderer::Gl

mod engine;
mod fallback;
mod feed;
mod overlay;
mod widget;

use std::path::{Path, PathBuf};

use emusic_ui::player_api::PlayerApi;
use emusic_ui::state::projectm::{PresetRequest, ProjectMAvailability, ProjectMSettings};
use win32ui::prelude::*;

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
}

/// The projectM surface: a GL custom widget plus its feed, settings and preset
/// requests. Host it like any other control and drive it from the shell tick.
pub struct ProjectMView {
    custom: Custom<ProjectMWidget, Msg>,
}

impl ProjectMView {
    /// Creates the surface. Its widget paints a placeholder until projectM can
    /// start, so construction never needs the DLLs or a GL context.
    pub fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let widget = ProjectMWidget::new(ui.dpi());
        Ok(Self {
            custom: Custom::new(ui, widget)?,
        })
    }

    /// Maps the surface's hover/double-click [`ProjectMGesture`]s to an app
    /// message. Without it the gestures are dropped.
    #[must_use]
    pub fn with_gestures(self, map: impl Fn(ProjectMGesture) -> Option<Msg> + 'static) -> Self {
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

    /// Reads the player for this frame, feeding samples while it plays and
    /// silence while it is paused or stopped.
    pub fn feed(&self, player: &dyn PlayerApi) {
        self.custom.widget().borrow().feed(player);
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

impl AsControl for ProjectMView {
    fn control(&self) -> &Control {
        self.custom.control()
    }
}
