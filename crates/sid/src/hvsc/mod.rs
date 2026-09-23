#![forbid(unsafe_code)]

//! Optional lookups into a local [HVSC](https://hvsc.c64.org) installation:
//! per-subtune song lengths (`DOCUMENTS/Songlengths.md5`) and the SID Tune
//! Information List (`DOCUMENTS/STIL.txt`).
//!
//! The index is keyed by the MD5 of the whole SID file for song lengths, and
//! by the HVSC-relative path for STIL, matching the on-disk formats. Build it
//! once from the user's HVSC folder and share it via `Arc`.

mod songlengths;
mod stil;

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use md5::{Digest, Md5};

/// Something went wrong reading an HVSC folder.
#[derive(Debug, thiserror::Error)]
pub enum HvscError {
    /// The folder doesn't look like HVSC: no song-lengths database was found.
    #[error("no Songlengths database under {0}")]
    Missing(String),
    /// A file couldn't be read.
    #[error("failed to read {path}: {error}")]
    Io {
        /// The file that failed.
        path: String,
        /// The underlying I/O error.
        error: std::io::Error,
    },
}

/// Lookup tables parsed from an HVSC `DOCUMENTS` folder.
pub struct HvscIndex {
    /// MD5 of a whole SID file -> per-subtune lengths (subtune 1 first).
    song_lengths: HashMap<[u8; 16], Vec<Duration>>,
    /// Normalised HVSC-relative path -> STIL comment text.
    stil: HashMap<String, String>,
}

impl HvscIndex {
    /// Loads the index from an HVSC root (the folder containing `DOCUMENTS/`).
    ///
    /// The song-lengths database is required; `STIL.txt` is optional.
    pub fn load(root: &Path) -> Result<Self, HvscError> {
        let documents = root.join("DOCUMENTS");
        let song_lengths_path = ["Songlengths.md5", "Songlengths.txt"]
            .iter()
            .map(|name| documents.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| HvscError::Missing(root.display().to_string()))?;
        let song_lengths = songlengths::parse(&read(&song_lengths_path)?);

        let stil_path = documents.join("STIL.txt");
        let stil = if stil_path.is_file() {
            stil::parse(&read(&stil_path)?)
        } else {
            HashMap::new()
        };

        Ok(Self {
            song_lengths,
            stil,
        })
    }

    /// The per-subtune lengths for `data` (whole SID file), if known.
    pub fn lengths(&self, data: &[u8]) -> Option<&[Duration]> {
        let digest: [u8; 16] = Md5::digest(data).into();
        self.song_lengths.get(&digest).map(Vec::as_slice)
    }

    /// The length of subtune `subtune` (1-based) for `data`, if known.
    pub fn length(&self, data: &[u8], subtune: u16) -> Option<Duration> {
        let lengths = self.lengths(data)?;
        let index = usize::from(subtune.saturating_sub(1));
        lengths.get(index).copied()
    }

    /// The STIL comment for the SID at HVSC-relative `relative_path`, if any.
    ///
    /// `relative_path` may use either slash style; matching is case-insensitive.
    pub fn stil(&self, relative_path: &str) -> Option<&str> {
        self.stil.get(&normalize(relative_path)).map(String::as_str)
    }
}

/// Reads a text file, mapping I/O errors to [`HvscError`].
///
/// `STIL.txt` isn't valid UTF-8 (it predates that convention), so invalid
/// bytes are replaced rather than failing the whole index.
fn read(path: &Path) -> Result<String, HvscError> {
    let bytes = std::fs::read(path).map_err(|error| HvscError::Io {
        path: path.display().to_string(),
        error,
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Normalises a path for STIL lookups: forward slashes, lowercased, and a
/// single leading slash (STIL keys start with one; a stripped relative path
/// may not).
fn normalize(path: &str) -> String {
    let forward = path.replace('\\', "/");
    format!("/{}", forward.trim_start_matches('/').to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_unifies_separators_and_case() {
        assert_eq!(normalize(r"DEMOS\0-9\Tune.sid"), "/demos/0-9/tune.sid");
        assert_eq!(normalize("/Demos/0-9/Tune.sid"), "/demos/0-9/tune.sid");
    }

    #[test]
    fn loads_and_looks_up_a_small_hvsc_folder() {
        let root = std::env::temp_dir().join(format!("emusic-hvsc-{}", std::process::id()));
        let documents = root.join("DOCUMENTS");
        std::fs::create_dir_all(&documents).expect("create temp dir");

        let tune = b"PSID fake tune bytes";
        let digest: [u8; 16] = Md5::digest(tune).into();
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        std::fs::write(
            documents.join("Songlengths.md5"),
            format!("[Database]\n; /DEMOS/0-9/Tune.sid\n{hex}=1:02 0:05.5\n"),
        )
        .expect("write songlengths");
        std::fs::write(
            documents.join("STIL.txt"),
            "### /DEMOS/ ###\n\n/DEMOS/0-9/Tune.sid\nCOMMENT: A test tune.\n",
        )
        .expect("write stil");

        let index = HvscIndex::load(&root).expect("load");
        assert_eq!(
            index.length(tune, 1),
            Some(Duration::from_secs(62)),
            "first subtune"
        );
        assert_eq!(
            index.length(tune, 2),
            Some(Duration::from_millis(5_500)),
            "second subtune"
        );
        assert_eq!(index.length(tune, 3), None, "out of range");
        assert_eq!(
            index.stil(r"DEMOS\0-9\Tune.sid"),
            Some("A test tune.")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_without_songlengths_is_rejected() {
        let root = std::env::temp_dir().join(format!("emusic-hvsc-empty-{}", std::process::id()));
        std::fs::create_dir_all(root.join("DOCUMENTS")).expect("create temp dir");
        assert!(matches!(
            HvscIndex::load(&root),
            Err(HvscError::Missing(_))
        ));
        let _ = std::fs::remove_dir_all(&root);
    }
}
