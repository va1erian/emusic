#![forbid(unsafe_code)]

//! [`Instance`]: one running projectM visualization and its preset
//! playlist, bound to the OpenGL context it was created on.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::ProjectMError;
use crate::events::Event;
use crate::ffi::{Libs, RawInstance};
use crate::params::Parameters;

/// How many other presets the playlist tries when one fails to load.
const PRESET_RETRIES: u32 = 5;

/// A projectM instance plus its playlist.
///
/// Not `Send`: it must stay on the thread owning its OpenGL context. Calls
/// that reach OpenGL return [`ProjectMError::NoCurrentContext`] or
/// [`ProjectMError::WrongContext`] unless that context is current. Prefer
/// [`Instance::destroy`] with the context current; dropping it elsewhere
/// leaks the GPU resources (with a warning) rather than freeing them on the
/// wrong context.
pub struct Instance {
    libs: Arc<Libs>,
    raw: Option<RawInstance>,
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instance").finish_non_exhaustive()
    }
}

/// `path` as the UTF-8 projectM reads.
fn utf8(path: &Path) -> Result<&str, ProjectMError> {
    path.to_str()
        .ok_or_else(|| ProjectMError::InvalidString(path.display().to_string()))
}

impl Instance {
    pub(crate) fn create(
        libs: Arc<Libs>,
        width: usize,
        height: usize,
    ) -> Result<Self, ProjectMError> {
        let context = libs
            .current_context()
            .ok_or(ProjectMError::NoCurrentContext)?;
        let raw = RawInstance::create(&libs, context)?;
        raw.set_retry_count(&libs, PRESET_RETRIES);
        raw.set_window_size(&libs, width, height);
        raw.set_parameters(&libs, &Parameters::default());
        Ok(Self {
            libs,
            raw: Some(raw),
        })
    }

    /// The live instance. Only [`Instance::destroy`] and `Drop` take it out,
    /// and both consume `self`, so it is always present here.
    fn raw(&self) -> &RawInstance {
        self.raw
            .as_ref()
            .expect("the instance is only taken out when `self` is consumed")
    }

    /// The live instance, once its context is checked to be current.
    fn gl(&self) -> Result<&RawInstance, ProjectMError> {
        let raw = self.raw();
        match self.libs.current_context() {
            None => Err(ProjectMError::NoCurrentContext),
            Some(context) if context != raw.context => Err(ProjectMError::WrongContext),
            Some(_) => Ok(raw),
        }
    }

    /// Feeds interleaved stereo samples (`[l, r, l, r, ..]`, -1.0..=1.0).
    /// Longer blocks are split to projectM's buffer size; an odd trailing
    /// sample is ignored.
    pub fn add_pcm(&self, samples: &[f32]) {
        let chunk = (self.libs.max_samples() * 2).max(2);
        for block in samples.chunks(chunk) {
            self.raw().add_pcm_stereo(&self.libs, block);
        }
    }

    /// Applies preset timing, cut and sensitivity settings.
    pub fn set_parameters(&self, params: &Parameters) {
        self.raw().set_parameters(&self.libs, params);
    }

    /// Resizes the render target to the framebuffer's size in pixels.
    pub fn resize(&self, width: usize, height: usize) -> Result<(), ProjectMError> {
        self.gl()?.set_window_size(&self.libs, width, height);
        Ok(())
    }

    /// Renders one frame into the current framebuffer.
    pub fn render(&self) -> Result<(), ProjectMError> {
        self.gl()?.render_frame(&self.libs);
        Ok(())
    }

    /// Sets the folders preset textures are looked up in.
    pub fn set_texture_paths(&self, paths: &[&Path]) -> Result<(), ProjectMError> {
        let paths = paths
            .iter()
            .map(|path| utf8(path))
            .collect::<Result<Vec<_>, _>>()?;
        self.gl()?.set_texture_paths(&self.libs, &paths)
    }

    /// Adds every preset under `dir` (recursively when `recurse`) to the
    /// playlist; returns how many were added.
    pub fn add_presets(&self, dir: &Path, recurse: bool) -> Result<usize, ProjectMError> {
        self.raw().add_path(&self.libs, utf8(dir)?, recurse)
    }

    /// Removes every preset from the playlist.
    pub fn clear_presets(&self) {
        self.raw().clear(&self.libs);
    }

    /// Filters the playlist with projectM's glob patterns (a leading `-`
    /// excludes, e.g. `-/presets/projectm-classic/*`); returns how many
    /// presets were removed.
    pub fn set_filter(&self, patterns: &[&str]) -> Result<usize, ProjectMError> {
        self.raw().set_filter(&self.libs, patterns)
    }

    /// Number of presets in the playlist.
    pub fn preset_count(&self) -> usize {
        self.raw().preset_count(&self.libs)
    }

    /// The playlist index currently shown.
    pub fn position(&self) -> usize {
        self.raw().position(&self.libs)
    }

    /// The preset file at playlist `index`.
    pub fn preset_path(&self, index: usize) -> Option<PathBuf> {
        self.raw().item(&self.libs, index)
    }

    /// Shows the next preset (a random one while shuffling).
    pub fn play_next(&self, hard_cut: bool) -> Result<usize, ProjectMError> {
        Ok(self.gl()?.play_next(&self.libs, hard_cut))
    }

    /// Shows the previously shown preset.
    pub fn play_previous(&self, hard_cut: bool) -> Result<usize, ProjectMError> {
        Ok(self.gl()?.play_previous(&self.libs, hard_cut))
    }

    /// Shows the preset at playlist `index`.
    pub fn play_index(&self, index: usize, hard_cut: bool) -> Result<usize, ProjectMError> {
        Ok(self.gl()?.set_position(&self.libs, index, hard_cut))
    }

    /// Loads a preset file directly, outside the playlist.
    pub fn load_preset(&self, path: &Path, smooth: bool) -> Result<(), ProjectMError> {
        self.gl()?.load_preset(&self.libs, utf8(path)?, smooth)
    }

    /// Takes the playlist events queued since the last call, oldest first.
    pub fn take_events(&self) -> Vec<Event> {
        self.raw().take_events()
    }

    /// Frees the instance and its GPU resources. Its context must be
    /// current; otherwise the instance is handed back with the error so the
    /// caller can retry once it is.
    pub fn destroy(mut self) -> Result<(), (Self, ProjectMError)> {
        if let Err(err) = self.gl() {
            return Err((self, err));
        }
        if let Some(raw) = self.raw.take() {
            raw.destroy(&self.libs);
        }
        Ok(())
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if self.raw.is_none() {
            return;
        }
        match self.gl() {
            Ok(_) => {
                if let Some(raw) = self.raw.take() {
                    raw.destroy(&self.libs);
                }
            }
            Err(err) => {
                tracing::warn!(%err, "projectM instance dropped off its context; leaking it");
                // Deliberately not destroyed: that would free GL objects on
                // the wrong (or no) context.
                std::mem::forget(self.raw.take());
            }
        }
    }
}
