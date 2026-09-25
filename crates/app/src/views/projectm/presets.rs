#![forbid(unsafe_code)]

//! Preset and texture discovery for the projectM surface (#299, #305).
//!
//! The install layout is `<exe>/visualizations/presets/<pack>/` plus
//! `<exe>/visualizations/textures/`; the user can add one folder of their own.
//! [`PresetRoots`] resolves which folders to use (skipping disabled packs) and
//! [`PresetScanner`] walks them on a background thread, so the UI thread never
//! scans thousands of `.milk` files: it only receives the ready file list.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use emusic_ui::state::projectm::ProjectMSettings;

/// The install layout under the executable.
const VISUALIZATIONS_DIR: &str = "visualizations";
const PRESETS_DIR: &str = "presets";
const TEXTURES_DIR: &str = "textures";
/// projectM presets are MilkDrop `.milk` files.
const PRESET_EXTENSION: &str = "milk";

/// The folders a scan walks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PresetRoots {
    /// Preset pack folders and the user's folder, scanned recursively.
    pub preset_dirs: Vec<PathBuf>,
    /// Folders projectM resolves preset textures in.
    pub texture_dirs: Vec<PathBuf>,
}

impl PresetRoots {
    /// Resolves the install layout under `exe_dir`, skipping packs disabled in
    /// `settings`, plus the user's optional folder.
    pub(crate) fn resolve(exe_dir: &Path, settings: &ProjectMSettings) -> Self {
        let base = exe_dir.join(VISUALIZATIONS_DIR);

        let mut preset_dirs = Vec::new();
        if let Ok(entries) = std::fs::read_dir(base.join(PRESETS_DIR)) {
            let mut packs: Vec<PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();
            packs.sort();
            for pack in packs {
                let enabled = pack
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_none_or(|name| settings.pack_enabled(name));
                if enabled {
                    preset_dirs.push(pack);
                }
            }
        }
        if let Some(user) = settings.user_preset_dir.as_ref().filter(|dir| dir.is_dir()) {
            preset_dirs.push(user.clone());
        }

        let textures = base.join(TEXTURES_DIR);
        let texture_dirs = textures.is_dir().then_some(textures).into_iter().collect();

        Self {
            preset_dirs,
            texture_dirs,
        }
    }
}

/// The files a scan found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PresetFiles {
    /// Every `.milk` file under the roots.
    pub presets: Vec<PathBuf>,
    /// The texture folders to hand to projectM.
    pub textures: Vec<PathBuf>,
}

/// Walks `roots` on a background thread and exposes the file list once ready.
pub(crate) struct PresetScanner {
    receiver: Receiver<PresetFiles>,
}

impl PresetScanner {
    /// Starts a scan. A thread that cannot be spawned yields no result, which
    /// the caller treats as "no presets found".
    pub(crate) fn spawn(roots: PresetRoots) -> Self {
        let (sender, receiver) = mpsc::channel();
        let _ = thread::Builder::new()
            .name("emusic-preset-scan".to_owned())
            .spawn(move || {
                let files = scan(&roots);
                // The receiver may already be gone (the surface was hidden and
                // the scanner dropped); the result is then simply discarded.
                let _ = sender.send(files);
            });
        Self { receiver }
    }

    /// The scan's result once it is ready, or `None` while it still runs.
    pub(crate) fn try_take(&self) -> Option<PresetFiles> {
        self.receiver.try_recv().ok()
    }
}

/// Collects the `.milk` files under `roots` and copies its texture folders.
fn scan(roots: &PresetRoots) -> PresetFiles {
    let mut presets = Vec::new();
    for dir in &roots.preset_dirs {
        collect_presets(dir, &mut presets);
    }
    PresetFiles {
        presets,
        textures: roots.texture_dirs.clone(),
    }
}

/// Appends every `.milk` file under `dir` (recursively) to `out`.
fn collect_presets(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_presets(&path, out);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(PRESET_EXTENSION))
        {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emusic_ui::state::projectm::ProjectMSettings;

    /// A scratch directory removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("emusic-projectm-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("create scratch dir");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn resolve_keeps_enabled_packs_and_the_user_folder() {
        let scratch = Scratch::new("resolve");
        let presets = scratch.path().join("visualizations/presets");
        std::fs::create_dir_all(presets.join("milkdrop-original")).expect("pack a");
        std::fs::create_dir_all(presets.join("projectm-classic")).expect("pack b");
        std::fs::create_dir_all(scratch.path().join("visualizations/textures")).expect("textures");
        let user = scratch.path().join("my-presets");
        std::fs::create_dir_all(&user).expect("user dir");

        let settings = ProjectMSettings {
            disabled_packs: vec!["projectm-classic".to_owned()],
            user_preset_dir: Some(user.clone()),
            ..ProjectMSettings::default()
        };
        let roots = PresetRoots::resolve(scratch.path(), &settings);

        assert_eq!(
            roots.preset_dirs,
            vec![presets.join("milkdrop-original"), user]
        );
        assert_eq!(
            roots.texture_dirs,
            vec![scratch.path().join("visualizations/textures")]
        );
    }

    #[test]
    fn scan_finds_milk_files_recursively_and_skips_others() {
        let scratch = Scratch::new("scan");
        let pack = scratch.path().join("pack");
        std::fs::create_dir_all(pack.join("nested")).expect("nested");
        std::fs::write(pack.join("a.milk"), b"x").expect("a");
        std::fs::write(pack.join("nested/b.MILK"), b"x").expect("b");
        std::fs::write(pack.join("notes.txt"), b"x").expect("txt");

        let roots = PresetRoots {
            preset_dirs: vec![pack],
            texture_dirs: Vec::new(),
        };
        let files = scan(&roots);

        let mut names: Vec<String> = files
            .presets
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["a.milk", "b.MILK"]);
    }

    #[test]
    fn resolve_without_an_install_layout_is_empty() {
        let scratch = Scratch::new("empty");
        let roots = PresetRoots::resolve(scratch.path(), &ProjectMSettings::default());
        assert_eq!(roots, PresetRoots::default());
    }
}
