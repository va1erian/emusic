#![forbid(unsafe_code)]

//! [`ProjectM`]: the loaded libraries, shared by every instance.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::ProjectMError;
use crate::ffi::{Libs, loader};
use crate::instance::Instance;

/// The loaded projectM libraries. Cheap to clone; instances keep it alive.
#[derive(Clone)]
pub struct ProjectM {
    libs: Arc<Libs>,
}

impl std::fmt::Debug for ProjectM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectM").finish_non_exhaustive()
    }
}

impl ProjectM {
    /// Loads the libraries from [`ProjectM::default_dir`].
    pub fn load() -> Result<Self, ProjectMError> {
        Self::load_from(&Self::default_dir()?)
    }

    /// Loads `glew32.dll` (when present), `projectM-4.dll` and
    /// `projectM-4-playlist.dll` from `dir`.
    pub fn load_from(dir: &Path) -> Result<Self, ProjectMError> {
        Ok(Self {
            libs: Arc::new(Libs::load(dir)?),
        })
    }

    /// `$EMUSIC_PROJECTM_DIR` if set, else `projectm\` next to the running
    /// executable.
    pub fn default_dir() -> Result<PathBuf, ProjectMError> {
        loader::projectm_dir()
    }

    /// The library's version string, e.g. `4.1.7`.
    pub fn version(&self) -> String {
        self.libs.version()
    }

    /// Whether an OpenGL context is current on this thread, i.e. whether
    /// [`ProjectM::create`] can succeed here.
    pub fn has_current_context(&self) -> bool {
        self.libs.current_context().is_some()
    }

    /// Creates an instance on the OpenGL context current on this thread
    /// (OpenGL 3.3 core). It stays bound to that context: rendering or
    /// destroying it needs the same context current again.
    pub fn create(&self, width: usize, height: usize) -> Result<Instance, ProjectMError> {
        Instance::create(Arc::clone(&self.libs), width, height)
    }
}
