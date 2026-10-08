//! [`LazyBackend`]: emusic's [`AudioBackend`] for MP3 files over an
//! [`OutputFactory`].

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use emusic_player::backend::{AudioBackend, BackendChannel};
use emusic_player::error::PlayerError;

use crate::channel::LazyChannel;
use crate::decode::Mp3Decoder;
use crate::output::OutputFactory;

/// Plays MP3 files through the streams an [`OutputFactory`] opens. Any other
/// file fails to open, so the queue skips it.
pub struct LazyBackend {
    factory: Arc<dyn OutputFactory>,
}

impl LazyBackend {
    /// A backend whose channels play through `factory`'s streams.
    pub fn new(factory: Arc<dyn OutputFactory>) -> LazyBackend {
        LazyBackend { factory }
    }
}

impl AudioBackend for LazyBackend {
    fn open(&self, path: &Path) -> Result<Box<dyn BackendChannel>, PlayerError> {
        let file = File::open(path)
            .map_err(|error| PlayerError::ReadFailed(format!("{}: {error}", path.display())))?;
        let decoder = Mp3Decoder::open(BufReader::new(file))
            .map_err(|error| PlayerError::Backend(format!("{}: {error}", path.display())))?;
        Ok(Box::new(LazyChannel::new(
            decoder,
            Arc::clone(&self.factory),
        )?))
    }

    /// Tracker modules are not played by this backend.
    fn set_tracker_resampling_quality(&self, _quality: u8) -> Result<(), PlayerError> {
        Ok(())
    }
}
