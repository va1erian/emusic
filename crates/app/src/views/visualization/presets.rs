//! projectM preset discovery for the preset browser (#299, #338).
//!
//! The install layout is `<exe>/visualizations/presets/<pack>/` plus the user's
//! optional folder; [`PresetScanner`] walks them on a background thread so the
//! UI thread never scans thousands of `.milk` files — it is woken when the file
//! list is ready.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use emusic_ui::state::projectm::ProjectMSettings;
use emusic_ui::views::preset_browser::PresetEntry;
use emusic_ui::waker::{Waker, WakerHandle};

/// The install layout under the executable.
const VISUALIZATIONS_DIR: &str = "visualizations";
const PRESETS_DIR: &str = "presets";
/// projectM presets are MilkDrop `.milk` files.
const PRESET_EXTENSION: &str = "milk";

/// The preset files a scan found, in playlist order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresetFiles {
    /// Every `.milk` file under the roots, tagged with its pack folder.
    pub presets: Vec<PresetEntry>,
}

impl PresetFiles {
    /// The entries a mock run serves instead of a real scan (#338).
    /// Deterministic, so screenshots and tests are reproducible.
    pub fn placeholder() -> Self {
        let packs = ["cream-of-the-crop", "en-d", "milkdrop-original"];
        let presets = packs
            .iter()
            .flat_map(|pack| {
                ["Alpha", "Dancer", "Pulse", "Zebra"].map(|name| {
                    PresetEntry::new(
                        PathBuf::from("visualizations")
                            .join("presets")
                            .join(pack)
                            .join(format!("{name}.milk")),
                        *pack,
                    )
                })
            })
            .collect();
        Self { presets }
    }
}

/// Walks the preset roots on a background thread and exposes the file list once
/// ready, waking the UI when it finishes.
pub struct PresetScanner {
    receiver: Receiver<PresetFiles>,
}

impl PresetScanner {
    /// Starts a scan. A thread that cannot be spawned yields no result, which
    /// the caller treats as "no presets found".
    pub fn spawn(exe_dir: &Path, settings: &ProjectMSettings, waker: WakerHandle) -> Self {
        let roots = preset_roots(exe_dir, settings);
        let (sender, receiver) = mpsc::channel();
        let _ = thread::Builder::new()
            .name("emusic-preset-scan".to_owned())
            .spawn(move || {
                let files = scan(&roots);
                // The receiver may already be gone (the view was dropped); the
                // result is then simply discarded.
                let _ = sender.send(files);
                waker.wake();
            });
        Self { receiver }
    }

    /// The scan's result once it is ready, or `None` while it still runs.
    pub fn try_take(&self) -> Option<PresetFiles> {
        self.receiver.try_recv().ok()
    }
}

/// Resolves the folders a scan walks: every enabled pack under the install
/// layout, plus the user's optional folder.
fn preset_roots(exe_dir: &Path, settings: &ProjectMSettings) -> Vec<PathBuf> {
    let base = exe_dir.join(VISUALIZATIONS_DIR).join(PRESETS_DIR);
    let mut roots = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base) {
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
                roots.push(pack);
            }
        }
    }
    if let Some(user) = settings.user_preset_dir.as_ref().filter(|dir| dir.is_dir()) {
        roots.push(user.clone());
    }
    roots
}

/// Collects the `.milk` files under `roots`.
fn scan(roots: &[PathBuf]) -> PresetFiles {
    let mut presets = Vec::new();
    for dir in roots {
        collect_presets(dir, &pack_name(dir), &mut presets);
    }
    PresetFiles { presets }
}

/// The pack folder's display name: its last path component, or the whole path
/// when it has none.
fn pack_name(dir: &Path) -> String {
    dir.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.to_string_lossy().into_owned())
}

/// Appends every `.milk` file under `dir` (recursively) to `out`, tagging each
/// with the root `pack` it was found under.
fn collect_presets(dir: &Path, pack: &str, out: &mut Vec<PresetEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_presets(&path, pack, out);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(PRESET_EXTENSION))
        {
            out.push(PresetEntry::new(path, pack));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "emusic-visualization-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("create scratch dir");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scan_finds_milk_files_recursively_and_skips_others() {
        let scratch = Scratch::new("scan");
        let pack = scratch.0.join("pack");
        std::fs::create_dir_all(pack.join("nested")).expect("nested");
        std::fs::write(pack.join("a.milk"), b"x").expect("a");
        std::fs::write(pack.join("nested/b.MILK"), b"x").expect("b");
        std::fs::write(pack.join("notes.txt"), b"x").expect("txt");

        let files = scan(&[pack]);
        let mut names: Vec<String> = files
            .presets
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
        assert!(files.presets.iter().all(|entry| entry.pack == "pack"));
    }

    #[test]
    fn resolve_keeps_enabled_packs_and_the_user_folder() {
        let scratch = Scratch::new("resolve");
        let presets = scratch.0.join("visualizations/presets");
        std::fs::create_dir_all(presets.join("milkdrop-original")).expect("pack a");
        std::fs::create_dir_all(presets.join("projectm-classic")).expect("pack b");
        let user = scratch.0.join("my-presets");
        std::fs::create_dir_all(&user).expect("user dir");

        let settings = ProjectMSettings {
            disabled_packs: vec!["projectm-classic".to_owned()],
            user_preset_dir: Some(user.clone()),
            ..ProjectMSettings::default()
        };
        let roots = preset_roots(&scratch.0, &settings);
        assert_eq!(
            roots,
            vec![presets.join("milkdrop-original"), user],
            "disabled packs are skipped, the user folder is kept"
        );
    }

    #[test]
    fn placeholder_presets_carry_a_name_and_pack() {
        let files = PresetFiles::placeholder();
        assert!(!files.presets.is_empty());
        assert!(
            files
                .presets
                .iter()
                .all(|entry| !entry.name.is_empty() && !entry.pack.is_empty())
        );
    }
}
