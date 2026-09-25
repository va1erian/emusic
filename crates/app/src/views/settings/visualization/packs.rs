#![forbid(unsafe_code)]

//! Known projectM preset packs and their on-disk status (#306).
//!
//! The install layout is `<exe>/visualizations/presets/<pack>/`, the same
//! [`PresetRoots`](crate::views::projectm::PresetRoots) resolves. Counting the
//! packs can walk thousands of files, so [`count_installed`] is only ever run
//! on the background thread the page spawns.

use std::path::{Path, PathBuf};

/// The preset-pack folders the Settings → Visualization page lists, in display
/// order. `milkdrop-texture-pack` is textures, not presets, so it is not here.
pub(super) const KNOWN_PACKS: [&str; 4] = [
    "milkdrop-original",
    "cream-of-the-crop",
    "en-d",
    "projectm-classic",
];

const VISUALIZATIONS_DIR: &str = "visualizations";
const PRESETS_DIR: &str = "presets";
const PRESET_EXTENSION: &str = "milk";

/// The `<exe>/visualizations/presets` directory.
pub(super) fn presets_root(exe_dir: &Path) -> PathBuf {
    exe_dir.join(VISUALIZATIONS_DIR).join(PRESETS_DIR)
}

/// The folder of one preset pack.
pub(super) fn pack_dir(exe_dir: &Path, pack: &str) -> PathBuf {
    presets_root(exe_dir).join(pack)
}

/// Whether the pack folder exists under the install layout.
pub(super) fn is_installed(exe_dir: &Path, pack: &str) -> bool {
    pack_dir(exe_dir, pack).is_dir()
}

/// Counts the `.milk` files of every known pack, recursively, on the calling
/// (background) thread.
pub(super) fn count_installed(exe_dir: &Path) -> Vec<(String, usize)> {
    KNOWN_PACKS
        .iter()
        .map(|pack| ((*pack).to_owned(), count_presets(&pack_dir(exe_dir, pack))))
        .collect()
}

/// Counts the `.milk` files under `dir` (recursively); a missing or unreadable
/// directory counts zero.
fn count_presets(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                count_presets(&path)
            } else {
                usize::from(
                    path.extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case(PRESET_EXTENSION)),
                )
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("emusic-viz-packs-{name}-{}", std::process::id()));
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
    fn counts_milk_files_recursively_and_ignores_others() {
        let scratch = Scratch::new("count");
        let pack = scratch.0.join("visualizations/presets/en-d/nested");
        std::fs::create_dir_all(&pack).expect("create pack");
        std::fs::write(pack.join("a.milk"), b"x").expect("a");
        std::fs::write(pack.join("b.MILK"), b"x").expect("b");
        std::fs::write(pack.join("notes.txt"), b"x").expect("txt");

        assert_eq!(
            count_presets(&scratch.0.join("visualizations/presets/en-d")),
            2
        );
    }

    #[test]
    fn missing_pack_counts_zero_and_reports_not_installed() {
        let scratch = Scratch::new("missing");
        assert!(!is_installed(&scratch.0, "cream-of-the-crop"));
        assert_eq!(count_presets(&pack_dir(&scratch.0, "cream-of-the-crop")), 0);
    }

    #[test]
    fn count_installed_lists_every_known_pack() {
        let scratch = Scratch::new("all");
        let pack = scratch.0.join("visualizations/presets/milkdrop-original");
        std::fs::create_dir_all(&pack).expect("create pack");
        std::fs::write(pack.join("one.milk"), b"x").expect("preset");

        let counts = count_installed(&scratch.0);
        assert_eq!(counts.len(), KNOWN_PACKS.len());
        assert_eq!(counts[0], ("milkdrop-original".to_owned(), 1));
        assert_eq!(counts[1].1, 0);
    }
}
