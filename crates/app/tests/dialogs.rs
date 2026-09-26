//! Secondary-dialog integration checks (#376): opens each dialog as a real
//! non-modal `xui_core` window against the mock library, drives its action or
//! close message, and verifies it closes.
//!
//! A test that leaves a window open would hang the runner, so every window is
//! closed and every test has two safeguards: a repeating timer that advances
//! phases (open -> act -> assert closed -> quit) and a worker-thread watchdog
//! that posts [`Msg::Quit`] after a bounded time. A failure therefore ends the
//! loop instead of waiting for input.

#![cfg(windows)]

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use emusic::app::Msg;
use emusic::dialogs::{database_info, properties, tag_editor};
use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::mock::MockLibrary;
use emusic_ui::tag_editor::{Status, TagEditorState};
use xui::xui_core::app::{App, Ui, WindowHandle};
use xui::xui_core::backend::{Backend, PlatformSpec, Result as BackendResult, TimerId};

/// How long one phase waits before acting, in milliseconds.
const TICK_MS: u32 = 40;
/// How long the watchdog waits before quitting, in seconds.
const WATCHDOG_SECS: u64 = 10;

/// A handle to whichever dialog a test opened.
enum Handle {
    Database(WindowHandle<database_info::Msg>),
    Properties(WindowHandle<properties::Msg>),
    Tags(WindowHandle<tag_editor::Msg>),
}

impl Handle {
    fn is_open(&self) -> bool {
        match self {
            Handle::Database(handle) => handle.is_open(),
            Handle::Properties(handle) => handle.is_open(),
            Handle::Tags(handle) => handle.is_open(),
        }
    }

    /// Sends the dialog the intent that should make it close itself.
    fn act(&self) {
        match self {
            Handle::Database(handle) => handle.send(database_info::Msg::Rescan),
            Handle::Properties(handle) => handle.send(properties::Msg::Close),
            Handle::Tags(handle) => handle.send(tag_editor::Msg::Close),
        }
    }

    fn close(&self) {
        match self {
            Handle::Database(handle) => handle.close(),
            Handle::Properties(handle) => handle.close(),
            Handle::Tags(handle) => handle.close(),
        }
    }
}

/// The test app: opens one dialog, then on each tick advances a phase.
struct DialogHarness {
    handle: Handle,
    timer: Option<TimerId>,
    phase: u8,
    opened: Rc<Cell<bool>>,
    closed: Rc<Cell<bool>>,
}

impl DialogHarness {
    fn finish(&mut self, ui: &Ui<Msg>) {
        if let Some(id) = self.timer.take() {
            ui.kill_timer(id);
        }
        self.handle.close();
        ui.quit();
    }
}

impl App for DialogHarness {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Timer => match self.phase {
                0 => {
                    self.opened.set(self.handle.is_open());
                    self.handle.act();
                    self.phase = 1;
                }
                1 => {
                    self.closed.set(!self.handle.is_open());
                    self.finish(ui);
                }
                _ => {}
            },
            // The watchdog, or an unexpected app message: end the loop.
            _ => self.finish(ui),
        }
    }
}

/// Runs `open` on a fresh window and returns whether it opened, then closed;
/// `None` when the session cannot create windows at all.
fn exercise(
    open: impl FnOnce(&Ui<Msg>) -> BackendResult<Handle> + 'static,
) -> Option<(bool, bool)> {
    let opened = Rc::new(Cell::new(false));
    let closed = Rc::new(Cell::new(false));
    let opened_for_make = Rc::clone(&opened);
    let closed_for_make = Rc::clone(&closed);

    let backend: Rc<dyn Backend> = Rc::new(xui::xui_win32::Win32Backend::new());
    let result = xui::xui_core::run_app(backend, PlatformSpec::new("dialog test"), move |ui| {
        let handle = open(ui).expect("open the dialog window");
        ui.on_timer(|_| Some(Msg::Timer));
        let timer = ui.set_timer(TICK_MS);
        // A worker threads pokes `Msg::Quit` if a phase never reaches the end.
        let proxy = ui.proxy();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(WATCHDOG_SECS));
            let _ = proxy.send(Msg::Quit);
        });
        DialogHarness {
            handle,
            timer: Some(timer),
            phase: 0,
            opened: opened_for_make,
            closed: closed_for_make,
        }
    });
    if result.is_err() {
        eprintln!("skipping: this session cannot create windows");
        return None;
    }
    Some((opened.get(), closed.get()))
}

/// The Properties dialog opens and closes on its Close intent.
#[test]
fn properties_opens_and_closes() {
    let Some((opened, closed)) = exercise(|ui| {
        let track = MockLibrary::new()
            .tracks()
            .first()
            .cloned()
            .unwrap_or_default();
        properties::open(ui, &track).map(Handle::Properties)
    }) else {
        return;
    };
    assert!(opened, "the Properties dialog opened");
    assert!(closed, "the Properties dialog closed on its Close intent");
}

/// The Database info dialog opens and closes on its Rescan intent.
#[test]
fn database_info_opens_and_closes() {
    let Some((opened, closed)) = exercise(|ui| {
        let library = MockLibrary::new();
        database_info::open(ui, &library).map(Handle::Database)
    }) else {
        return;
    };
    assert!(opened, "the Database info dialog opened");
    assert!(
        closed,
        "the Database info dialog closed on its Rescan intent"
    );
}

/// The tag editor opens and closes on its Close intent.
#[test]
fn tag_editor_opens_and_closes() {
    let Some((opened, closed)) = exercise(|ui| {
        let track = MockLibrary::new()
            .tracks()
            .first()
            .cloned()
            .unwrap_or_default();
        let state = TagEditorState::new(&track);
        let bridge = Rc::new(std::cell::RefCell::new(tag_editor::Bridge::new(
            Status::Editing,
        )));
        tag_editor::open(ui, &state, bridge, ui.proxy()).map(Handle::Tags)
    }) else {
        return;
    };
    assert!(opened, "the tag editor opened");
    assert!(closed, "the tag editor closed on its Close intent");
}
