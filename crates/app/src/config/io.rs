//! Disk I/O for [`Config`]: default path, tolerant loading, and atomic
//! (temp file + rename) saving.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::warn;

use super::Config;

/// Errors from [`load`] and [`save`]. [`load`] already implements the
/// issue's recovery policy (defaults + `.bak`), so callers only see these
/// from [`save`].
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Parse(#[from] toml::de::Error),
    #[error("{0}")]
    Serialize(#[from] toml::ser::Error),
}

/// `%APPDATA%\emusic\config.toml` (`dirs::config_dir` maps to `%APPDATA%`
/// on Windows). `None` only if the OS provides no home directory.
pub fn config_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("emusic").join("config.toml"))
}

/// Loads the config from `path`, falling back to [`Config::default`] when
/// there is no file, and when the file cannot be parsed (keeping a
/// `config.toml.bak` of the bad file first).
pub fn load(path: &Path) -> Config {
    match load_inner(path) {
        Ok(config) => config,
        Err(err) => {
            warn!(
                %err,
                path = %path.display(),
                "falling back to default config"
            );
            Config::default()
        }
    }
}

fn load_inner(path: &Path) -> Result<Config, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(err) => return Err(err.into()),
    };
    match toml::from_str(&text) {
        Ok(config) => Ok(config),
        Err(err) => {
            backup_bad_file(path);
            Err(err.into())
        }
    }
}

/// Best effort: renames the unparsable file to `config.toml.bak` so the
/// user's hand-edits are not lost; only logs on failure.
fn backup_bad_file(path: &Path) {
    let bak = path.with_extension("toml.bak");
    match fs::rename(path, &bak) {
        Ok(()) => warn!(
            backup = %bak.display(),
            "kept a backup of the unparsable config"
        ),
        Err(err) => warn!(%err, "could not back up the unparsable config"),
    }
}

/// Writes the config atomically: serialized to a `config.toml.tmp` sibling
/// first, then renamed over the real file, so a crash mid-write can never
/// leave a half-written config behind.
pub fn save(path: &Path, config: &Config) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(config)?;
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text)?;
    // `fs::rename` replaces an existing destination on both Windows
    // (MoveFileEx with REPLACE_EXISTING) and Unix, making this atomic.
    fs::rename(&tmp, path)?;
    Ok(())
}
