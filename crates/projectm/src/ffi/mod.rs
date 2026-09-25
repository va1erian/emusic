//! The only module in this crate containing `unsafe`: the loaded libraries,
//! their resolved entry points, and [`RawInstance`], an owned projectM
//! instance plus its playlist. The safe API in the rest of the crate never
//! touches a raw pointer.
//!
//! Every call here that reaches OpenGL (creating, rendering, destroying)
//! requires the instance's context to be current. [`RawInstance`] records
//! the context it was created on and the safe layer checks it with
//! [`Libs::current_context`] before each such call.

pub mod loader;
mod raw;

use std::cell::Cell;
use std::ffi::{CStr, CString, c_char, c_uint, c_void};
use std::path::Path;
use std::ptr::NonNull;

use libloading::Library;

use crate::error::ProjectMError;
use crate::events::Event;

/// The loaded DLLs and their entry points. Immutable once loaded.
pub struct Libs {
    pub(crate) core: raw::Core,
    pub(crate) playlist: raw::Playlist,
    glew: Option<raw::Glew>,
    #[cfg(windows)]
    get_current_context: raw::GetCurrentContextFn,
    // Keep the DLLs mapped for as long as the tables above point into them.
    // Declared last so they drop after nothing can call through them.
    _libraries: Vec<Library>,
}

// SAFETY: every field is written once in `Libs::load` and only read after:
// function pointers into DLLs that stay mapped while `Libs` lives, and the
// address of GLEW's `glewExperimental`, written only by `init_gl_loader`
// right before `glewInit` on the thread whose context is current (the only
// thread that may drive projectM at that moment).
unsafe impl Send for Libs {}
// SAFETY: see `Send`: shared access only reads immutable data.
unsafe impl Sync for Libs {}

impl Libs {
    /// Loads GLEW (when present), the core and the playlist library from
    /// `dir`, in dependency order.
    pub fn load(dir: &Path) -> Result<Self, ProjectMError> {
        let mut libraries = Vec::new();
        let glew = match loader::load_from(dir, loader::GLEW_DLL) {
            Ok(library) => {
                let glew = raw::Glew::load(&library)?;
                libraries.push(library);
                Some(glew)
            }
            Err(err) => {
                tracing::debug!(%err, "no GLEW next to projectM; assuming it needs none");
                None
            }
        };
        let core_library = loader::load_from(dir, loader::CORE_DLL)?;
        let core = raw::Core::load(&core_library)?;
        libraries.push(core_library);
        let playlist_library = loader::load_from(dir, loader::PLAYLIST_DLL)?;
        let playlist = raw::Playlist::load(&playlist_library)?;
        libraries.push(playlist_library);
        #[cfg(windows)]
        let get_current_context = {
            let opengl = loader::load_system("opengl32.dll")?;
            // SAFETY: `wglGetCurrentContext` is exported by opengl32.dll
            // with exactly this signature.
            let get = unsafe { opengl.get::<raw::GetCurrentContextFn>(b"wglGetCurrentContext\0") }
                .map(|symbol| *symbol)
                .map_err(|_| ProjectMError::MissingSymbol("wglGetCurrentContext".into()))?;
            libraries.push(opengl);
            get
        };
        Ok(Self {
            core,
            playlist,
            glew,
            #[cfg(windows)]
            get_current_context,
            _libraries: libraries,
        })
    }

    /// An opaque id for the OpenGL context current on this thread, if any.
    pub fn current_context(&self) -> Option<usize> {
        #[cfg(windows)]
        {
            // SAFETY: `wglGetCurrentContext` takes no arguments and only
            // reads thread-local state.
            let context = unsafe { (self.get_current_context)() };
            (!context.is_null()).then_some(context as usize)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// Initialises GLEW against the current context, when projectM uses it.
    /// The caller has checked a context is current.
    fn init_gl_loader(&self) -> Result<(), ProjectMError> {
        let Some(glew) = &self.glew else {
            return Ok(());
        };
        // SAFETY: `experimental` is the address of GLEW's `GLboolean
        // glewExperimental` global, valid while glew32.dll is loaded; writing
        // GL_TRUE (1) before `glewInit` is GLEW's documented usage. A context
        // is current on this thread, as `glewInit` requires.
        let code = unsafe {
            glew.experimental.write(1);
            (glew.init)()
        };
        if code == 0 {
            Ok(())
        } else {
            Err(ProjectMError::GlewInit(code))
        }
    }

    /// projectM's version string, e.g. `4.1.7`.
    pub fn version(&self) -> String {
        // SAFETY: returns a newly allocated NUL-terminated string (or null)
        // that must be released with `projectm_free_string`, done below.
        unsafe {
            let raw = (self.core.get_version_string)();
            if raw.is_null() {
                return String::new();
            }
            let version = CStr::from_ptr(raw).to_string_lossy().into_owned();
            (self.core.free_string)(raw);
            version
        }
    }

    /// The most samples per channel one `add_pcm` call keeps.
    pub fn max_samples(&self) -> usize {
        // SAFETY: a pure query with no arguments.
        unsafe { (self.core.pcm_get_max_samples)() as usize }
    }
}

/// Callback events waiting for [`RawInstance::take_events`]. Boxed so its
/// address, handed to projectM as `user_data`, never moves.
#[derive(Default)]
struct EventSink {
    events: Cell<Vec<Event>>,
}

impl EventSink {
    fn push(&self, event: Event) {
        let mut events = self.events.take();
        events.push(event);
        self.events.set(events);
    }
}

extern "C" fn on_switched(is_hard_cut: bool, index: c_uint, user: *mut c_void) {
    // SAFETY: `user` is the `EventSink` registered in `RawInstance::create`,
    // which outlives the playlist (callbacks are cleared before it drops).
    let sink = unsafe { &*(user as *const EventSink) };
    sink.push(Event::PresetSwitched {
        index: index as usize,
        hard_cut: is_hard_cut,
    });
}

extern "C" fn on_failed(file: *const c_char, message: *const c_char, user: *mut c_void) {
    // SAFETY: as in `on_switched`; projectM passes NUL-terminated strings
    // valid for the duration of the call (or null).
    let (sink, file, message) = unsafe {
        let text = |ptr: *const c_char| {
            if ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };
        (&*(user as *const EventSink), text(file), text(message))
    };
    sink.push(Event::PresetFailed { file, message });
}

/// An owned projectM instance and its connected playlist.
///
/// Raw pointers keep it `!Send`/`!Sync`: projectM must stay on the thread
/// that owns its OpenGL context.
pub struct RawInstance {
    handle: NonNull<c_void>,
    playlist: NonNull<c_void>,
    sink: Box<EventSink>,
    /// The context it was created on; every GL call needs it current.
    pub(crate) context: usize,
}

/// Converts a path or name into a C string projectM can read (UTF-8; see
/// the crate docs about the process code page).
pub(crate) fn c_string(text: &str) -> Result<CString, ProjectMError> {
    CString::new(text).map_err(|_| ProjectMError::InvalidString(text.to_string()))
}

impl RawInstance {
    /// Creates an instance and a playlist on the current context, whose id
    /// the caller has read from [`Libs::current_context`].
    pub fn create(libs: &Libs, context: usize) -> Result<Self, ProjectMError> {
        libs.init_gl_loader()?;
        // SAFETY: a context is current (checked by the caller) and GLEW, if
        // used, is initialised, as `projectm_create` requires.
        let handle = NonNull::new(unsafe { (libs.core.create)() })
            .ok_or(ProjectMError::CreateFailed("an instance"))?;
        // SAFETY: `handle` is a live instance.
        let playlist = match NonNull::new(unsafe { (libs.playlist.create)(handle.as_ptr()) }) {
            Some(playlist) => playlist,
            None => {
                // SAFETY: `handle` is live and unused elsewhere; the context
                // is still current.
                unsafe { (libs.core.destroy)(handle.as_ptr()) };
                return Err(ProjectMError::CreateFailed("a playlist"));
            }
        };
        let sink = Box::<EventSink>::default();
        let user = (&*sink as *const EventSink).cast_mut().cast::<c_void>();
        // SAFETY: `playlist` is live; `user` points at the boxed sink, which
        // this struct owns and keeps until after the callbacks are cleared.
        unsafe {
            (libs.playlist.set_switched_callback)(playlist.as_ptr(), Some(on_switched), user);
            (libs.playlist.set_failed_callback)(playlist.as_ptr(), Some(on_failed), user);
        }
        Ok(Self {
            handle,
            playlist,
            sink,
            context,
        })
    }

    /// Takes the callback events queued since the last call.
    pub fn take_events(&self) -> Vec<Event> {
        self.sink.events.take()
    }

    /// Frees the playlist and the instance. The instance's context must be
    /// current (checked by the caller).
    pub fn destroy(self, libs: &Libs) {
        // SAFETY: both handles are live and owned by `self`, consumed here so
        // they can't be used again; the context is current. Callbacks are
        // cleared first so nothing reaches the sink after this.
        unsafe {
            let playlist = self.playlist.as_ptr();
            (libs.playlist.set_switched_callback)(playlist, None, std::ptr::null_mut());
            (libs.playlist.set_failed_callback)(playlist, None, std::ptr::null_mut());
            (libs.playlist.destroy)(playlist);
            (libs.core.destroy)(self.handle.as_ptr());
        }
    }
}

mod calls;
