//! The raw projectM, playlist and GLEW entry points, resolved once at load
//! time. Signatures follow the libprojectM 4.1 C headers.

use std::ffi::{c_char, c_int, c_uint, c_void};

use libloading::Library;

use crate::error::ProjectMError;

/// Opaque `projectm_handle`.
pub type Handle = *mut c_void;
/// Opaque `projectm_playlist_handle`.
pub type PlaylistHandle = *mut c_void;

/// `projectm_playlist_preset_switched_event`.
pub type SwitchedFn = extern "C" fn(is_hard_cut: bool, index: c_uint, user: *mut c_void);
/// `projectm_playlist_preset_switch_failed_event`.
pub type FailedFn = extern "C" fn(file: *const c_char, message: *const c_char, user: *mut c_void);

/// Resolves `name` from `library` as a `T` (a function pointer type).
fn sym<T: Copy>(library: &Library, name: &str) -> Result<T, ProjectMError> {
    let mut bytes = name.as_bytes().to_vec();
    bytes.push(0);
    // SAFETY: every caller names a projectM/GLEW export and asks for the
    // exact type the C headers declare for it, so reinterpreting the
    // symbol's address as `T` is sound. The returned copy stays valid while
    // `library` stays loaded, which the owning `Libs` guarantees.
    unsafe { library.get::<T>(&bytes) }
        .map(|symbol| *symbol)
        .map_err(|_| ProjectMError::MissingSymbol(name.to_string()))
}

/// Core library entry points.
pub struct Core {
    pub create: unsafe extern "C" fn() -> Handle,
    pub destroy: unsafe extern "C" fn(Handle),
    pub load_preset_file: unsafe extern "C" fn(Handle, *const c_char, bool),
    pub get_version_string: unsafe extern "C" fn() -> *mut c_char,
    pub free_string: unsafe extern "C" fn(*const c_char),
    pub pcm_get_max_samples: unsafe extern "C" fn() -> c_uint,
    pub pcm_add_float: unsafe extern "C" fn(Handle, *const f32, c_uint, c_int),
    pub set_texture_search_paths: unsafe extern "C" fn(Handle, *const *const c_char, usize),
    pub set_beat_sensitivity: unsafe extern "C" fn(Handle, f32),
    pub set_hard_cut_enabled: unsafe extern "C" fn(Handle, bool),
    pub set_hard_cut_sensitivity: unsafe extern "C" fn(Handle, f32),
    pub set_soft_cut_duration: unsafe extern "C" fn(Handle, f64),
    pub set_preset_duration: unsafe extern "C" fn(Handle, f64),
    pub set_fps: unsafe extern "C" fn(Handle, i32),
    pub set_preset_locked: unsafe extern "C" fn(Handle, bool),
    pub set_window_size: unsafe extern "C" fn(Handle, usize, usize),
    pub render_frame: unsafe extern "C" fn(Handle),
}

impl Core {
    pub fn load(library: &Library) -> Result<Self, ProjectMError> {
        Ok(Self {
            create: sym(library, "projectm_create")?,
            destroy: sym(library, "projectm_destroy")?,
            load_preset_file: sym(library, "projectm_load_preset_file")?,
            get_version_string: sym(library, "projectm_get_version_string")?,
            free_string: sym(library, "projectm_free_string")?,
            pcm_get_max_samples: sym(library, "projectm_pcm_get_max_samples")?,
            pcm_add_float: sym(library, "projectm_pcm_add_float")?,
            set_texture_search_paths: sym(library, "projectm_set_texture_search_paths")?,
            set_beat_sensitivity: sym(library, "projectm_set_beat_sensitivity")?,
            set_hard_cut_enabled: sym(library, "projectm_set_hard_cut_enabled")?,
            set_hard_cut_sensitivity: sym(library, "projectm_set_hard_cut_sensitivity")?,
            set_soft_cut_duration: sym(library, "projectm_set_soft_cut_duration")?,
            set_preset_duration: sym(library, "projectm_set_preset_duration")?,
            set_fps: sym(library, "projectm_set_fps")?,
            set_preset_locked: sym(library, "projectm_set_preset_locked")?,
            set_window_size: sym(library, "projectm_set_window_size")?,
            render_frame: sym(library, "projectm_opengl_render_frame")?,
        })
    }
}

/// Playlist library entry points.
pub struct Playlist {
    pub create: unsafe extern "C" fn(Handle) -> PlaylistHandle,
    pub destroy: unsafe extern "C" fn(PlaylistHandle),
    pub size: unsafe extern "C" fn(PlaylistHandle) -> u32,
    pub clear: unsafe extern "C" fn(PlaylistHandle),
    pub item: unsafe extern "C" fn(PlaylistHandle, u32) -> *mut c_char,
    pub free_string: unsafe extern "C" fn(*mut c_char),
    pub add_path: unsafe extern "C" fn(PlaylistHandle, *const c_char, bool, bool) -> u32,
    pub set_shuffle: unsafe extern "C" fn(PlaylistHandle, bool),
    pub set_retry_count: unsafe extern "C" fn(PlaylistHandle, u32),
    pub set_position: unsafe extern "C" fn(PlaylistHandle, u32, bool) -> u32,
    pub get_position: unsafe extern "C" fn(PlaylistHandle) -> u32,
    pub play_next: unsafe extern "C" fn(PlaylistHandle, bool) -> u32,
    pub play_previous: unsafe extern "C" fn(PlaylistHandle, bool) -> u32,
    pub set_filter: unsafe extern "C" fn(PlaylistHandle, *const *const c_char, usize),
    pub apply_filter: unsafe extern "C" fn(PlaylistHandle) -> usize,
    pub set_switched_callback:
        unsafe extern "C" fn(PlaylistHandle, Option<SwitchedFn>, *mut c_void),
    pub set_failed_callback: unsafe extern "C" fn(PlaylistHandle, Option<FailedFn>, *mut c_void),
}

impl Playlist {
    pub fn load(library: &Library) -> Result<Self, ProjectMError> {
        Ok(Self {
            create: sym(library, "projectm_playlist_create")?,
            destroy: sym(library, "projectm_playlist_destroy")?,
            size: sym(library, "projectm_playlist_size")?,
            clear: sym(library, "projectm_playlist_clear")?,
            item: sym(library, "projectm_playlist_item")?,
            free_string: sym(library, "projectm_playlist_free_string")?,
            add_path: sym(library, "projectm_playlist_add_path")?,
            set_shuffle: sym(library, "projectm_playlist_set_shuffle")?,
            set_retry_count: sym(library, "projectm_playlist_set_retry_count")?,
            set_position: sym(library, "projectm_playlist_set_position")?,
            get_position: sym(library, "projectm_playlist_get_position")?,
            play_next: sym(library, "projectm_playlist_play_next")?,
            play_previous: sym(library, "projectm_playlist_play_previous")?,
            set_filter: sym(library, "projectm_playlist_set_filter")?,
            apply_filter: sym(library, "projectm_playlist_apply_filter")?,
            set_switched_callback: sym(
                library,
                "projectm_playlist_set_preset_switched_event_callback",
            )?,
            set_failed_callback: sym(
                library,
                "projectm_playlist_set_preset_switch_failed_event_callback",
            )?,
        })
    }
}

/// GLEW, which projectM 4.1 on Windows uses to resolve OpenGL functions.
/// Its function pointers are process-wide globals inside `glew32.dll`, so
/// the host initialises them once a context is current.
pub struct Glew {
    pub init: unsafe extern "system" fn() -> c_uint,
    /// `glewExperimental`: must be set so a core-profile context gets every
    /// entry point.
    pub experimental: *mut u8,
}

impl Glew {
    pub fn load(library: &Library) -> Result<Self, ProjectMError> {
        Ok(Self {
            init: sym(library, "glewInit")?,
            experimental: sym(library, "glewExperimental")?,
        })
    }
}

/// `wglGetCurrentContext`, to check which OpenGL context is current.
#[cfg(windows)]
pub type GetCurrentContextFn = unsafe extern "system" fn() -> *mut c_void;
