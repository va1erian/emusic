//! Directory tree grouping for the library index.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use emusic_core::Track;

use super::natural_sort::natural_compare;

/// A node in the directory tree.
#[derive(Debug, Clone, PartialEq)]
pub struct DirNode {
    /// Full path of this directory.
    pub path: PathBuf,
    /// Display name (the last path component, or the drive root).
    pub name: String,
    /// Child directories, sorted by natural-order name.
    pub children: Vec<DirNode>,
    /// Indices of tracks in [`super::LibraryIndex::tracks`] located directly
    /// in this directory (not in subdirectories).
    pub track_slots: Vec<usize>,
}

impl DirNode {
    /// Total number of tracks in this directory, not including children.
    pub fn direct_track_count(&self) -> usize {
        self.track_slots.len()
    }

    /// Total number of tracks in this directory and all descendants.
    pub fn total_track_count(&self) -> usize {
        let child_count: usize = self.children.iter().map(|c| c.total_track_count()).sum();
        self.track_slots.len() + child_count
    }
}

/// Internal builder for the directory tree.
#[derive(Debug, Default)]
pub struct Dirs {
    /// Map from directory path to its accumulator.
    nodes: HashMap<PathBuf, Accumulator>,
}

#[derive(Debug)]
struct Accumulator {
    path: PathBuf,
    track_slots: Vec<usize>,
    child_paths: HashSet<PathBuf>,
}

impl Dirs {
    /// Registers a track in the directory tree.
    pub fn add(&mut self, track: &Track, slot: usize) {
        let dir = track.dir.clone();
        self.node_for(&dir).track_slots.push(slot);

        let mut current = dir;
        while let Some(parent) = current.parent() {
            if parent.as_os_str().is_empty() {
                break;
            }
            let parent_buf = parent.to_path_buf();
            let entry = self
                .nodes
                .entry(parent_buf.clone())
                .or_insert_with(|| Accumulator {
                    path: parent_buf.clone(),
                    track_slots: Vec::new(),
                    child_paths: HashSet::new(),
                });
            entry.child_paths.insert(current);
            current = parent_buf;
        }
    }

    /// Removes a track from the directory tree.
    pub fn remove(&mut self, track: &Track, slot: usize) {
        let dir = &track.dir;
        if let Some(entry) = self.nodes.get_mut(dir) {
            let before = entry.track_slots.len();
            entry.track_slots.retain(|&s| s != slot);
            debug_assert!(
                entry.track_slots.len() < before,
                "slot {slot} not found in directory '{}'",
                dir.display()
            );
        }
    }

    /// Returns the root directory nodes of the tree.
    ///
    /// A node is considered a root if it has no parent in the index or its
    /// parent is the empty path.
    pub fn build(mut self) -> Vec<DirNode> {
        let mut roots: Vec<PathBuf> = self
            .nodes
            .keys()
            .filter(|path| is_root(path, &self.nodes))
            .cloned()
            .collect();
        roots.sort_by(|a, b| natural_compare(&dir_name(a), &dir_name(b)));

        roots
            .into_iter()
            .map(|path| self.build_node(&path))
            .collect()
    }

    /// Rebuilds from scratch when the index is fully reset.
    pub fn rebuild(&mut self, tracks: &[Option<Track>]) {
        self.nodes.clear();
        for (slot, track) in tracks.iter().enumerate() {
            if let Some(track) = track {
                self.add(track, slot);
            }
        }
    }

    fn node_for(&mut self, path: &Path) -> &mut Accumulator {
        self.nodes
            .entry(path.to_path_buf())
            .or_insert_with(|| Accumulator {
                path: path.to_path_buf(),
                track_slots: Vec::new(),
                child_paths: HashSet::new(),
            })
    }

    fn build_node(&mut self, path: &Path) -> DirNode {
        let acc = self.nodes.remove(path).unwrap_or_else(|| Accumulator {
            path: path.to_path_buf(),
            track_slots: Vec::new(),
            child_paths: HashSet::new(),
        });

        let mut children: Vec<DirNode> = acc
            .child_paths
            .into_iter()
            .map(|child| self.build_node(&child))
            .collect();
        children.sort_by(|a, b| natural_compare(&a.name, &b.name));

        DirNode {
            name: dir_name(&acc.path),
            path: acc.path,
            children,
            track_slots: acc.track_slots,
        }
    }
}

fn is_root(path: &Path, nodes: &HashMap<PathBuf, Accumulator>) -> bool {
    match path.parent() {
        None => true,
        Some(parent) => parent.as_os_str().is_empty() || !nodes.contains_key(parent),
    }
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.as_os_str().to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use emusic_core::{Track, TrackId, TrackKind};

    use super::*;

    fn track(path: &str) -> Track {
        let path_str = if cfg!(not(windows)) {
            path.replace('\\', "/")
        } else {
            path.to_string()
        };
        let path = PathBuf::from(&path_str);
        let dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/"));
        Track {
            id: TrackId::UNASSIGNED,
            path,
            dir,
            filename: "t.flac".to_string(),
            ext: "flac".to_string(),
            size: 1,
            mtime: 1,
            kind: TrackKind::Stream,
            duration_ms: 60_000,
            bitrate: None,
            sample_rate: None,
            channels: None,
            title: None,
            artist: None,
            album_artist: None,
            album: None,
            genre: None,
            year: None,
            track_no: None,
            disc_no: None,
            composer: None,
            comment: None,
            art_source: emusic_core::ArtSource::None,
            added_at: 1,
            starred: false,
        }
    }

    #[test]
    fn builds_nested_tree() {
        let mut dirs = Dirs::default();
        dirs.add(&track(r"C:\music\Artist\Album\1.flac"), 0);
        dirs.add(&track(r"C:\music\Artist\Album\2.flac"), 1);
        dirs.add(&track(r"C:\music\Other\3.flac"), 2);

        let roots = dirs.build();
        assert_eq!(roots.len(), 1);
        assert!(roots[0].name == r"C:\" || roots[0].name == "C:" || roots[0].name == "C");
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[0].children[0].name, "music");
        assert_eq!(roots[0].children[0].children.len(), 2);
        assert_eq!(roots[0].total_track_count(), 3);
    }

    #[test]
    fn direct_tracks_not_in_children() {
        let mut dirs = Dirs::default();
        dirs.add(&track(r"C:\music\Artist\Album\1.flac"), 0);
        dirs.add(&track(r"C:\music\Artist\2.flac"), 1);

        let roots = dirs.build();
        let music = &roots[0].children[0];
        let artist = &music.children[0];
        assert_eq!(artist.direct_track_count(), 1);
        assert_eq!(artist.children[0].direct_track_count(), 1);
        assert_eq!(artist.total_track_count(), 2);
    }

    #[test]
    fn remove_decreases_count() {
        let mut dirs = Dirs::default();
        let t = track(r"C:\music\a.flac");
        dirs.add(&t, 0);
        dirs.remove(&t, 0);
        let roots = dirs.build();
        assert_eq!(roots[0].direct_track_count(), 0);
    }
}
