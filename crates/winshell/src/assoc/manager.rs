//! Registering, unregistering and probing emusic's per-user file
//! associations, entirely through the safe `windows-registry` crate.

use std::path::{Path, PathBuf};

#[cfg(windows)]
use windows_registry::{CURRENT_USER, Type};

use super::roots::AssocRoots;
use crate::error::Result;
use crate::sys;

/// Extensions emusic can be associated with: the formats BASS (and its
/// plugins) can play, plus the Commodore 64 SID formats the app's own decoder
/// handles.
pub const EXTENSIONS: &[&str] = &[
    "mp3", "mp2", "m4a", "aac", "flac", "ogg", "oga", "opus", "wav", "aiff", "aif", "wma", "wv",
    "ape", "mpc", "webm", "mka", "mod", "s3m", "xm", "it", "mtm", "umx", "mo3", "mid", "midi",
    "sid", "psid", "rsid",
];

/// Subdirectory of the install dir holding the per-extension `.ico` files
/// shipped alongside the exe (#125). Their location is fixed by the installer
/// (`installer/emusic.iss`) and resolved relative to the exe at registration
/// time.
const ICONS_DIR: &str = "icons";

/// Icon file used for every extension that has no dedicated `file-<ext>.ico`.
const FALLBACK_ICON: &str = "file-audio.ico";

/// Registers/unregisters emusic as a per-user file association handler.
pub struct AssocManager {
    roots: AssocRoots,
    app_name: String,
    exe_key: String,
}

impl AssocManager {
    /// A manager writing to the real, production registry locations.
    pub fn new(app_name: impl Into<String>) -> Self {
        let app_name = app_name.into();
        let roots = AssocRoots::production(&app_name);
        Self::with_roots(app_name, roots)
    }

    /// A manager confined to `roots` — used by tests to avoid touching the
    /// real associations.
    pub fn with_roots(app_name: impl Into<String>, roots: AssocRoots) -> Self {
        let app_name = app_name.into();
        let exe_key = format!("{app_name}.exe");
        Self {
            roots,
            app_name,
            exe_key,
        }
    }

    fn progid(&self, ext: &str) -> String {
        format!("{}.{}", self.app_name, ext)
    }

    #[cfg(windows)]
    pub fn register(&self, exe: &Path, exts: &[&str]) -> Result<()> {
        let icons_dir = exe
            .parent()
            .map(|dir| dir.join(ICONS_DIR))
            .filter(|dir| dir.is_dir());
        let exe = exe.display().to_string();
        self.register_application(&exe)?;
        for ext in exts {
            self.register_extension(&exe, icons_dir.as_deref(), ext)?;
        }
        self.register_capabilities(exts)?;
        sys::notify_assoc_changed();
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn register(&self, _exe: &Path, _exts: &[&str]) -> Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    fn register_application(&self, exe: &str) -> Result<()> {
        let app = CURRENT_USER.create(format!(
            r"{}\Applications\{}",
            self.roots.classes, self.exe_key
        ))?;
        app.set_string("FriendlyAppName", &self.app_name)?;

        CURRENT_USER
            .create(format!(
                r"{}\Applications\{}\shell\open\command",
                self.roots.classes, self.exe_key
            ))?
            .set_string("", format!("\"{exe}\" \"%1\""))?;

        Ok(())
    }

    #[cfg(windows)]
    fn register_extension(&self, exe: &str, icons_dir: Option<&Path>, ext: &str) -> Result<()> {
        let progid = self.progid(ext);
        let classes = &self.roots.classes;
        let icon = match icons_dir {
            Some(dir) => format!("\"{}\"", icon_file(dir, ext).display()),
            None => format!("\"{exe}\",0"),
        };

        let progid_key = CURRENT_USER.create(format!(r"{classes}\{progid}"))?;
        progid_key.set_string("", format!("emusic {} File", ext.to_uppercase()))?;
        CURRENT_USER
            .create(format!(r"{classes}\{progid}\DefaultIcon"))?
            .set_string("", icon)?;
        CURRENT_USER
            .create(format!(r"{classes}\{progid}\shell\open\command"))?
            .set_string("", format!("\"{exe}\" \"%1\""))?;
        CURRENT_USER
            .create(format!(r"{classes}\{progid}\shell\enqueue"))?
            .set_string("", "Add to emusic queue")?;
        CURRENT_USER
            .create(format!(r"{classes}\{progid}\shell\enqueue\command"))?
            .set_string("", format!("\"{exe}\" --enqueue \"%1\""))?;

        CURRENT_USER
            .create(format!(r"{classes}\.{ext}\OpenWithProgids"))?
            .set_bytes(&progid, Type::Other(0), &[])?;

        CURRENT_USER
            .create(format!(
                r"{classes}\Applications\{}\SupportedTypes",
                self.exe_key
            ))?
            .set_string(format!(".{ext}"), "")?;

        Ok(())
    }

    #[cfg(windows)]
    fn register_capabilities(&self, exts: &[&str]) -> Result<()> {
        let app = &self.roots.app;

        let caps = CURRENT_USER.create(format!(r"{app}\Capabilities"))?;
        caps.set_string("ApplicationName", "emusic")?;
        caps.set_string("ApplicationDescription", "emusic music player and library")?;

        let file_assoc = CURRENT_USER.create(format!(r"{app}\Capabilities\FileAssociations"))?;
        for ext in exts {
            file_assoc.set_string(format!(".{ext}"), self.progid(ext))?;
        }

        CURRENT_USER
            .create(&self.roots.registered_apps)?
            .set_string(&self.app_name, format!(r"{app}\Capabilities"))?;

        Ok(())
    }

    /// Whether `ext` currently has an emusic ProgID registered.
    #[cfg(windows)]
    pub fn is_registered(&self, ext: &str) -> bool {
        CURRENT_USER
            .open(format!(r"{}\{}", self.roots.classes, self.progid(ext)))
            .is_ok()
    }

    #[cfg(not(windows))]
    pub fn is_registered(&self, _ext: &str) -> bool {
        false
    }

    /// Removes everything [`register`](Self::register) may have written.
    #[cfg(windows)]
    pub fn unregister(&self) -> Result<()> {
        let classes = &self.roots.classes;
        for ext in EXTENSIONS {
            let progid = self.progid(ext);
            let _ = CURRENT_USER.remove_tree(format!(r"{classes}\{progid}"));
            if let Ok(open_with) = CURRENT_USER.open(format!(r"{classes}\.{ext}\OpenWithProgids")) {
                let _ = open_with.remove_value(&progid);
            }
        }
        let _ = CURRENT_USER.remove_tree(format!(r"{classes}\Applications\{}", self.exe_key));
        let _ = CURRENT_USER.remove_tree(&self.roots.app);
        if let Ok(registered) = CURRENT_USER.open(&self.roots.registered_apps) {
            let _ = registered.remove_value(&self.app_name);
        }
        sys::notify_assoc_changed();
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn unregister(&self) -> Result<()> {
        Ok(())
    }
}

/// The icon file `ext`'s `DefaultIcon` should point at, given the install's
/// `icons` directory: the extension's own `file-<ext>.ico` when present,
/// otherwise the generic `file-audio.ico` fallback.
fn icon_file(icons_dir: &Path, ext: &str) -> PathBuf {
    let dedicated = icons_dir.join(format!("file-{ext}.ico"));
    if dedicated.is_file() {
        dedicated
    } else {
        icons_dir.join(FALLBACK_ICON)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(case: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("winshell-icons-{case}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn touch(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, b"ico").expect("write temp icon");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn dedicated_icon_wins_when_present() {
        let dir = TempDir::new("dedicated");
        let icon = dir.touch("file-flac.ico");
        assert_eq!(icon_file(&dir.0, "flac"), icon);
    }

    #[test]
    fn falls_back_to_file_audio_when_no_dedicated_icon() {
        let dir = TempDir::new("fallback");
        let fallback = dir.touch(FALLBACK_ICON);
        assert_eq!(icon_file(&dir.0, "webm"), fallback);
    }

    #[test]
    fn missing_directory_still_yields_the_fallback_path() {
        let dir = std::env::temp_dir().join("winshell-icons-does-not-exist");
        assert_eq!(icon_file(&dir, "mp3"), dir.join(FALLBACK_ICON));
    }
}
