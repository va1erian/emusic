#![forbid(unsafe_code)]

//! The independent projectM window (#303): a non-modal, owned top-level window
//! hosting one [`ProjectMView`] that fills its client area.
//!
//! Both the main window and this one run their own `App` and queue on the
//! process's single message loop. The main app keeps a [`WindowHandle`] and
//! drives the child by sending [`VizWindowMsg`]s (settings, presets, PCM); the
//! child forwards gestures and engine events back to the main app through a
//! [`Proxy`]. Closing the window is intercepted and forwarded as a visibility
//! change, so the main app hides it and its state — and placement — survive a
//! show/hide cycle.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use emusic_ui::state::WindowGeometry;
use emusic_ui::state::projectm::{PresetRequest, ProjectMSettings, VizCommand, VizDock};
use win32ui::prelude::*;
use win32ui::{column, dip};

use super::presets::PresetFiles;
use super::{ProjectMEvent, ProjectMGesture, ProjectMView};
use crate::app::Msg;

/// The window caption; the shown preset is appended as a subtitle.
const TITLE: &str = "emusic - Visualization";

/// A message to the visualization window's own app.
pub(crate) enum VizWindowMsg {
    /// Replace the engine and preset settings.
    Settings(Box<ProjectMSettings>),
    /// Set the preset restored the next time an instance is created.
    LastPreset(Option<PathBuf>),
    /// Queue a preset navigation request.
    Preset(PresetRequest),
    /// Replace the scanned preset file list.
    Presets(PresetFiles),
    /// Feed interleaved stereo PCM.
    Samples(Vec<f32>),
    /// Feed a block of silence.
    Silence,
    /// Start or stop the surface's work, without closing the window.
    Visible(bool),
    /// Free the projectM instance, keeping the window.
    Suspend,
    /// Follow a live theme change.
    Theme(Theme),
    /// The user clicked the close button: hide the visualization, keep the
    /// window.
    CloseRequested,
    /// Forward a message to the main app's queue.
    Main(Msg),
}

/// The child window's app: the surface plus the proxy back to the main app.
struct VizWindowApp {
    view: ProjectMView<VizWindowMsg>,
    main: Proxy<Msg>,
}

impl VizWindowApp {
    /// Builds the surface filling the client area, intercepts the close button
    /// so the window is hidden rather than destroyed, and wires the surface's
    /// gestures to [`VizWindowMsg`].
    fn new(ui: &mut Ui<VizWindowMsg>, main: Proxy<Msg>) -> Self {
        let view = ProjectMView::new(ui)
            .expect("create the visualization widget")
            .with_gestures(forward_gesture);
        ui.set_layout(column![view.fill(1)]);
        ui.on_close(|| Some(VizWindowMsg::CloseRequested));
        Self { view, main }
    }

    /// Sends a message to the main app, ignoring a closed main window.
    fn forward(&self, msg: Msg) {
        let _ = self.main.send(msg);
    }

    /// Forwards the events raised since the last update: the shown preset
    /// (which updates the caption) and the engine availability.
    fn drain_events(&self, ui: &Ui<VizWindowMsg>) {
        for event in self.view.take_events() {
            match event {
                ProjectMEvent::PresetShown(path) => {
                    let stem = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("preset");
                    ui.set_title(&format!("{TITLE} - {stem}"));
                    self.forward(Msg::Viz(VizCommand::PresetShown(path)));
                }
                ProjectMEvent::AvailabilityChanged(availability) => {
                    self.forward(Msg::VizAvailability(availability));
                }
            }
        }
    }
}

impl App for VizWindowApp {
    type Msg = VizWindowMsg;

    fn update(&mut self, msg: VizWindowMsg, ui: &mut Ui<VizWindowMsg>) {
        match msg {
            VizWindowMsg::Settings(settings) => self.view.set_settings(&settings),
            VizWindowMsg::LastPreset(path) => self.view.set_last_preset(path.as_deref()),
            VizWindowMsg::Preset(request) => self.view.request_preset(request),
            VizWindowMsg::Presets(files) => self.view.set_presets(files),
            VizWindowMsg::Samples(samples) => self.view.feed_samples(&samples),
            VizWindowMsg::Silence => self.view.push_silence(),
            VizWindowMsg::Visible(visible) => self.view.set_visible(visible),
            VizWindowMsg::Suspend => self.view.suspend(),
            VizWindowMsg::Theme(theme) => ui.set_theme(theme),
            VizWindowMsg::CloseRequested => self.forward(Msg::Viz(VizCommand::SetVisible(false))),
            VizWindowMsg::Main(message) => self.forward(message),
        }
        self.drain_events(ui);
    }
}

/// Maps a surface gesture onto the message that forwards it to the main app:
/// "pop out" docks back to the panel, double-click and the fullscreen button go
/// fullscreen, the hide button hides, and a right-click opens the main app's
/// preset menu.
fn forward_gesture(gesture: ProjectMGesture) -> Option<VizWindowMsg> {
    let command = match gesture {
        ProjectMGesture::PopOut => VizCommand::SetDock(VizDock::Panel),
        ProjectMGesture::Fullscreen | ProjectMGesture::DoubleClick => {
            VizCommand::SetFullscreen(true)
        }
        ProjectMGesture::Hide => VizCommand::SetVisible(false),
        ProjectMGesture::ContextMenu => return Some(VizWindowMsg::Main(Msg::VizMenu)),
    };
    Some(VizWindowMsg::Main(Msg::Viz(command)))
}

/// The independent visualization window: the handle the main app drives it
/// with, plus the DPI it was created for, to convert its geometry to the
/// logical points [`WindowGeometry`] is stored in.
pub(crate) struct VizWindow {
    handle: WindowHandle<VizWindowMsg>,
    dpi: u32,
}

impl VizWindow {
    /// Opens the window, restoring `saved` geometry (clamped to the work area)
    /// before it is first shown. The child inherits the opener's theme.
    pub fn open(ui: &Ui<Msg>, saved: WindowGeometry) -> win32ui::Result<Self> {
        let main = ui.proxy();
        let dpi = Rc::new(Cell::new(96));
        let dpi_for_make = Rc::clone(&dpi);
        let handle = ui.open_window(
            WindowSpec::new(TITLE)
                .size(dip(640.0), dip(480.0))
                .backdrop(Backdrop::Acrylic),
            move |child_ui| {
                dpi_for_make.set(child_ui.dpi());
                if let Some(placement) = saved_placement(&saved, child_ui.dpi()) {
                    let _ = child_ui.set_placement(&placement);
                }
                VizWindowApp::new(child_ui, main)
            },
        )?;
        Ok(Self {
            handle,
            dpi: dpi.get(),
        })
    }

    /// Whether the window still exists.
    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }

    /// Whether the window is currently shown.
    pub fn is_visible(&self) -> bool {
        self.handle.is_visible()
    }

    /// Shows the window and restarts the surface's work.
    pub fn show(&self) {
        self.send(VizWindowMsg::Visible(true));
        self.handle.show();
    }

    /// Hides the window and stops the surface's work, keeping both the instance
    /// (for the grace period, #305) and the window's state.
    pub fn hide(&self) {
        self.send(VizWindowMsg::Visible(false));
        self.handle.hide();
    }

    /// Tells the child to free its projectM instance once the grace period
    /// expired.
    pub fn suspend(&self) {
        self.send(VizWindowMsg::Suspend);
    }

    /// Replaces the engine and preset settings.
    pub fn set_settings(&self, settings: &ProjectMSettings) {
        self.send(VizWindowMsg::Settings(Box::new(settings.clone())));
    }

    /// Sets the preset to restore the next time an instance is created.
    pub fn set_last_preset(&self, path: Option<&Path>) {
        self.send(VizWindowMsg::LastPreset(path.map(Path::to_path_buf)));
    }

    /// Queues a preset navigation request.
    pub fn request_preset(&self, request: PresetRequest) {
        self.send(VizWindowMsg::Preset(request));
    }

    /// Replaces the scanned preset file list and texture folders.
    pub fn set_presets(&self, files: PresetFiles) {
        self.send(VizWindowMsg::Presets(files));
    }

    /// Feeds interleaved stereo PCM from the main app's player.
    pub fn feed_samples(&self, samples: &[f32]) {
        self.send(VizWindowMsg::Samples(samples.to_vec()));
    }

    /// Feeds a block of silence.
    pub fn push_silence(&self) {
        self.send(VizWindowMsg::Silence);
    }

    /// Follows a live theme change.
    pub fn set_theme(&self, theme: Theme) {
        self.send(VizWindowMsg::Theme(theme));
    }

    /// The window's current geometry in logical points, for persistence.
    pub fn geometry(&self) -> WindowGeometry {
        let placement = self.handle.placement();
        let normal = placement.normal;
        let scale = 96.0 / self.dpi as f32;
        WindowGeometry {
            size: Some([
                (normal.right - normal.left) as f32 * scale,
                (normal.bottom - normal.top) as f32 * scale,
            ]),
            position: Some([normal.left as f32 * scale, normal.top as f32 * scale]),
            maximized: placement.show == ShowState::Maximized,
        }
    }

    fn send(&self, msg: VizWindowMsg) {
        let _ = self.handle.send(msg);
    }
}

/// Builds the placement for saved `geometry`, in device pixels for `dpi`, and
/// clamps it to the current monitors. `None` when nothing was saved.
fn saved_placement(geometry: &WindowGeometry, dpi: u32) -> Option<Placement> {
    let (Some([width, height]), Some([left, top])) = (geometry.size, geometry.position) else {
        return None;
    };
    let scale = dpi as f32 / 96.0;
    let left = (left * scale).round() as i32;
    let top = (top * scale).round() as i32;
    let width = (width * scale).round().max(1.0) as i32;
    let height = (height * scale).round().max(1.0) as i32;
    let normal = Rect::new(left, top, left + width, top + height);
    let show = if geometry.maximized {
        ShowState::Maximized
    } else {
        ShowState::Normal
    };
    Some(Placement { normal, show }.clamp_to_work_areas())
}
