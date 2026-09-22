//! Builds the mock library's directory tree (#18) from the generated track
//! paths, mirroring what the real backend derives from `LibraryIndex`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::library_api::{DirNodeInfo, TrackInfo};

/// Groups `tracks` by their parent directory into a name-sorted tree with
/// per-directory direct and subtree track counts.
pub fn build(tracks: &[TrackInfo]) -> Vec<DirNodeInfo> {
    let mut nodes: HashMap<PathBuf, Node> = HashMap::new();
    for track in tracks {
        let Some(dir) = Path::new(&track.path).parent() else {
            continue;
        };
        let mut current = dir.to_path_buf();
        nodes.entry(current.clone()).or_default().direct += 1;
        while let Some(parent) = current.parent() {
            if parent.as_os_str().is_empty() {
                break;
            }
            let parent = parent.to_path_buf();
            nodes
                .entry(parent.clone())
                .or_default()
                .children
                .insert(current);
            current = parent;
        }
    }

    let mut roots: Vec<PathBuf> = nodes
        .keys()
        .filter(|path| !is_child_of_index(path, &nodes))
        .cloned()
        .collect();
    roots.sort_by(|a, b| compare_names(&name(a), &name(b)));

    roots.iter().map(|path| build_node(path, &nodes)).collect()
}

#[derive(Debug, Default)]
struct Node {
    direct: usize,
    children: HashSet<PathBuf>,
}

fn build_node(path: &Path, nodes: &HashMap<PathBuf, Node>) -> DirNodeInfo {
    let node = &nodes[path];
    let mut children: Vec<DirNodeInfo> = node
        .children
        .iter()
        .map(|child| build_node(child, nodes))
        .collect();
    children.sort_by(|a, b| compare_names(&a.name, &b.name));

    let total_track_count = node.direct
        + children
            .iter()
            .map(|child| child.total_track_count)
            .sum::<usize>();

    DirNodeInfo {
        path: path.to_string_lossy().into_owned(),
        name: name(path),
        direct_track_count: node.direct,
        total_track_count,
        children,
    }
}

/// Whether `path`'s parent is itself a node, i.e. `path` is not a root.
fn is_child_of_index(path: &Path, nodes: &HashMap<PathBuf, Node>) -> bool {
    path.parent()
        .is_some_and(|parent| nodes.contains_key(parent))
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.as_os_str().to_string_lossy().into_owned())
}

fn compare_names(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests;
