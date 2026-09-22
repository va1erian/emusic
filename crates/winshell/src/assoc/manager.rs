//! Registering, unregistering and probing emusic's per-user file
//! associations, entirely through the safe `windows-registry` crate.

use std::path::Path;

use windows_registry::{CURRENT_USER, Type};

use super::roots::AssocRoots;
use crate::error::Result;
use crate::sys;

/// Extensions emusic can be associated with. Matches the formats BASS (and
/// its plugins) can play.
pub const EXTENSIONS: &[&str] = &[
    "mp3", "mp2", "m4a", "aac", "flac", "ogg", "oga", "opus", "wav", "aiff", "aif", "wma", "wv",
    "ape", "mpc", "webm", "mka", "mod", "s3m", "xm", "it", "mtm", "umx", "mo3",
];

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

    /// Registers `exe` as the handler for `exts` (ProgIDs, icons, `shell
    /// open`/`enqueue` commands, `OpenWithProgids`, `SupportedTypes` and the
    /// app's `Capabilities`/`RegisteredApplications` entries), then notifies
    /// Explorer.
    pub fn register(&self, exe: &Path, exts: &[&str]) -> Result<()> {
        let exe = exe.display().to_string();
        for ext in exts {
            self.register_extension(&exe, ext)?;
        }
        self.register_capabilities(exts)?;
        sys::notify_assoc_changed();
        Ok(())
    }

    fn register_extension(&self, exe: &str, ext: &str) -> Result<()> {
        let progid = self.progid(ext);
        let classes = &self.roots.classes;

        let progid_key = CURRENT_USER.create(format!(r"{classes}\{progid}"))?;
        progid_key.set_string("", format!("emusic {} File", ext.to_uppercase()))?;
        CURRENT_USER
            .create(format!(r"{classes}\{progid}\DefaultIcon"))?
            .set_string("", format!("\"{exe}\",0"))?;
        CURRENT_USER
            .create(format!(r"{classes}\{progid}\shell\open\command"))?
            .set_string("", format!("\"{exe}\" \"%1\""))?;
        CURRENT_USER
            .create(format!(r"{classes}\{progid}\shell\enqueue"))?
            .set_string("", "Add to emusic queue")?;
        CURRENT_USER
            .create(format!(r"{classes}\{progid}\shell\enqueue\command"))?
            .set_string("", format!("\"{exe}\" --enqueue \"%1\""))?;

        // REG_NONE (type 0): the convention Explorer itself uses for
        // OpenWithProgids entries — the value only needs to exist.
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
    pub fn is_registered(&self, ext: &str) -> bool {
        CURRENT_USER
            .open(format!(r"{}\{}", self.roots.classes, self.progid(ext)))
            .is_ok()
    }

    /// Removes everything [`register`](Self::register) may have written,
    /// for every extension in [`EXTENSIONS`], then notifies Explorer.
    ///
    /// Best-effort and idempotent: missing keys/values are silently
    /// ignored so this is safe to call whether or not (or how much of)
    /// registration previously succeeded.
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
}
