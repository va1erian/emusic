//! Thin per-call wrappers over [`RawInstance`]'s handles. Each is a single
//! FFI call on live handles; calls marked "GL" may touch OpenGL, so the
//! safe layer checks the instance's context is current before making them.

use std::ffi::{CStr, c_char, c_int};
use std::path::PathBuf;

use super::{Libs, RawInstance, c_string};
use crate::error::ProjectMError;

/// `PROJECTM_STEREO`.
const STEREO: c_int = 2;

impl RawInstance {
    /// Queues interleaved stereo samples. No GL.
    pub fn add_pcm_stereo(&self, libs: &Libs, samples: &[f32]) {
        let frames = samples.len() / 2;
        if frames == 0 {
            return;
        }
        // SAFETY: the handle is live and `samples` holds `frames` complete
        // stereo frames; projectM copies what it reads before returning.
        unsafe {
            (libs.core.pcm_add_float)(
                self.handle.as_ptr(),
                samples.as_ptr(),
                frames as u32,
                STEREO,
            )
        };
    }

    /// Plain numeric parameters. No GL.
    pub fn set_parameters(&self, libs: &Libs, params: &crate::Parameters) {
        let handle = self.handle.as_ptr();
        // SAFETY: the handle is live; these setters only store values.
        unsafe {
            (libs.core.set_preset_duration)(handle, f64::from(params.preset_duration_secs));
            (libs.core.set_soft_cut_duration)(handle, f64::from(params.soft_cut_secs));
            (libs.core.set_hard_cut_enabled)(handle, params.hard_cuts);
            (libs.core.set_hard_cut_sensitivity)(handle, params.hard_cut_sensitivity);
            (libs.core.set_beat_sensitivity)(handle, params.beat_sensitivity);
            (libs.core.set_fps)(handle, params.fps as i32);
            (libs.core.set_preset_locked)(handle, params.preset_locked);
            (libs.playlist.set_shuffle)(self.playlist.as_ptr(), params.shuffle);
        }
    }

    /// GL: resizes the render target.
    pub fn set_window_size(&self, libs: &Libs, width: usize, height: usize) {
        // SAFETY: the handle is live; the caller checked the context.
        unsafe { (libs.core.set_window_size)(self.handle.as_ptr(), width.max(1), height.max(1)) };
    }

    /// GL: renders one frame into the current framebuffer.
    pub fn render_frame(&self, libs: &Libs) {
        // SAFETY: the handle is live; the caller checked the context.
        unsafe { (libs.core.render_frame)(self.handle.as_ptr()) };
    }

    /// GL: sets the folders textures are searched in.
    pub fn set_texture_paths(&self, libs: &Libs, paths: &[&str]) -> Result<(), ProjectMError> {
        let owned = paths
            .iter()
            .map(|path| c_string(path))
            .collect::<Result<Vec<_>, _>>()?;
        let pointers: Vec<*const c_char> = owned.iter().map(|path| path.as_ptr()).collect();
        // SAFETY: the handle is live; `pointers` holds `len` NUL-terminated
        // strings kept alive by `owned` for the call, which copies them.
        unsafe {
            (libs.core.set_texture_search_paths)(
                self.handle.as_ptr(),
                pointers.as_ptr(),
                pointers.len(),
            )
        };
        Ok(())
    }

    /// GL: loads one preset file directly.
    pub fn load_preset(&self, libs: &Libs, path: &str, smooth: bool) -> Result<(), ProjectMError> {
        let path = c_string(path)?;
        // SAFETY: the handle is live; `path` is NUL-terminated; the caller
        // checked the context.
        unsafe { (libs.core.load_preset_file)(self.handle.as_ptr(), path.as_ptr(), smooth) };
        Ok(())
    }

    /// Adds the presets under `path` to the playlist; returns how many.
    /// No GL (scans files only).
    pub fn add_path(&self, libs: &Libs, path: &str, recurse: bool) -> Result<usize, ProjectMError> {
        let path = c_string(path)?;
        // SAFETY: the playlist is live and `path` is NUL-terminated.
        let added = unsafe {
            (libs.playlist.add_path)(self.playlist.as_ptr(), path.as_ptr(), recurse, false)
        };
        Ok(added as usize)
    }

    /// Adds `files` to the playlist, skipping ones already present; returns how
    /// many were added. No GL (takes the file list ready-made).
    pub fn add_preset_files(&self, libs: &Libs, files: &[&str]) -> Result<usize, ProjectMError> {
        let owned = files
            .iter()
            .map(|file| c_string(file))
            .collect::<Result<Vec<_>, _>>()?;
        let pointers: Vec<*const c_char> = owned.iter().map(|file| file.as_ptr()).collect();
        let playlist = self.playlist.as_ptr();
        // SAFETY: the playlist is live; `pointers` holds `len` NUL-terminated
        // strings kept alive by `owned` for the call, which copies them.
        let added = unsafe {
            (libs.playlist.add_presets)(playlist, pointers.as_ptr(), pointers.len() as u32, false)
        };
        Ok(added as usize)
    }

    /// Empties the playlist. No GL.
    pub fn clear(&self, libs: &Libs) {
        // SAFETY: the playlist is live.
        unsafe { (libs.playlist.clear)(self.playlist.as_ptr()) };
    }

    /// Keeps only presets matching the glob-style `filters` (projectM's
    /// syntax: a leading `-` excludes). No GL.
    pub fn set_filter(&self, libs: &Libs, filters: &[&str]) -> Result<usize, ProjectMError> {
        let owned = filters
            .iter()
            .map(|filter| c_string(filter))
            .collect::<Result<Vec<_>, _>>()?;
        let pointers: Vec<*const c_char> = owned.iter().map(|filter| filter.as_ptr()).collect();
        let playlist = self.playlist.as_ptr();
        // SAFETY: the playlist is live; the strings outlive the call, which
        // copies them.
        let kept = unsafe {
            (libs.playlist.set_filter)(playlist, pointers.as_ptr(), pointers.len());
            (libs.playlist.apply_filter)(playlist)
        };
        Ok(kept)
    }

    /// Number of presets in the playlist. No GL.
    pub fn preset_count(&self, libs: &Libs) -> usize {
        // SAFETY: the playlist is live.
        unsafe { (libs.playlist.size)(self.playlist.as_ptr()) as usize }
    }

    /// The playlist's current index. No GL.
    pub fn position(&self, libs: &Libs) -> usize {
        // SAFETY: the playlist is live.
        unsafe { (libs.playlist.get_position)(self.playlist.as_ptr()) as usize }
    }

    /// The preset file at `index`, if any. No GL.
    pub fn item(&self, libs: &Libs, index: usize) -> Option<PathBuf> {
        let index = u32::try_from(index).ok()?;
        // SAFETY: the playlist is live; the returned string (or null) is
        // owned by us and released with `projectm_playlist_free_string`.
        unsafe {
            let raw = (libs.playlist.item)(self.playlist.as_ptr(), index);
            if raw.is_null() {
                return None;
            }
            let path = CStr::from_ptr(raw).to_string_lossy().into_owned();
            (libs.playlist.free_string)(raw);
            Some(PathBuf::from(path))
        }
    }

    /// GL: jumps to `index`, loading that preset.
    pub fn set_position(&self, libs: &Libs, index: usize, hard_cut: bool) -> usize {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        // SAFETY: the playlist is live; the caller checked the context.
        unsafe { (libs.playlist.set_position)(self.playlist.as_ptr(), index, hard_cut) as usize }
    }

    /// GL: advances to the next (or random, when shuffling) preset.
    pub fn play_next(&self, libs: &Libs, hard_cut: bool) -> usize {
        // SAFETY: the playlist is live; the caller checked the context.
        unsafe { (libs.playlist.play_next)(self.playlist.as_ptr(), hard_cut) as usize }
    }

    /// GL: goes back to the previously shown preset.
    pub fn play_previous(&self, libs: &Libs, hard_cut: bool) -> usize {
        // SAFETY: the playlist is live; the caller checked the context.
        unsafe { (libs.playlist.play_previous)(self.playlist.as_ptr(), hard_cut) as usize }
    }

    /// How many other presets to try when one fails to load. No GL.
    pub fn set_retry_count(&self, libs: &Libs, count: u32) {
        // SAFETY: the playlist is live.
        unsafe { (libs.playlist.set_retry_count)(self.playlist.as_ptr(), count) };
    }
}
