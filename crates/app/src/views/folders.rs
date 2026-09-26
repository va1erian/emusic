//! The Folders view (#101, #111), ported to the portable widget layer: a
//! lazily-loaded [`TreeView`] over the library's directory grouping, an
//! "include subfolders" [`CheckBox`] and the shared [`TrackView`].
//!
//! Selection, the include-subfolders filter and the selected folder's track
//! table live in `emusic_ui`'s
//! [`FoldersView`](emusic_ui::views::folders::FoldersView); this module only
//! owns the portable controls and maps their events to [`Msg`]s.
//!
//! The directory tree is a [`TreeModel`]: only the roots are read up front and
//! a branch's children are fetched the first time it expands, so an unopened
//! folder costs nothing. The tree's right-click "Shuffle play" menu is a
//! follow-up (the portable `TreeView` has no context-menu hook yet, and the
//! portable context menu is #376).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use emusic_ui::library_api::{DirNodeInfo, LibraryDataSource, TrackInfo};
use emusic_ui::state::Command;
use emusic_ui::views::folders::FoldersView as FoldersModel;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{CheckBox, NodeId, TreeModel, TreeNode, TreeView};

use crate::app::Msg;
use crate::views::track_table::TrackView;

/// The directory tree panel's width, in design units.
const TREE_WIDTH: f32 = 260.0;
/// The header row height reserved for the include-subfolders checkbox.
const HEADER_HEIGHT: f32 = 28.0;
/// The horizontal inset between the tree and the track table.
const INSET: f32 = 8.0;

/// One directory, flattened: its display data plus its children's indices into
/// [`DirTreeModel::nodes`]. Storing indices (not child slices) means building
/// the model never copies a subtree.
struct FlatDir {
    path: String,
    name: String,
    total_track_count: usize,
    has_children: bool,
    children: Vec<usize>,
}

impl FlatDir {
    /// The row text: the folder name and its total track count.
    fn label(&self) -> String {
        format!("{} ({})", self.name, self.total_track_count)
    }
}

/// A lazily-loaded [`TreeModel`] over the library's directory grouping.
///
/// The library snapshot is copied once into a flat array (the row strings
/// only); expanding a node then walks a handful of indices. `children` is only
/// called for the roots and for branches the control actually expands.
struct DirTreeModel {
    nodes: Vec<FlatDir>,
    roots: Vec<usize>,
}

impl DirTreeModel {
    /// Flattens `tree` into nodes (ids are node indices) and root indices.
    fn new(tree: &[DirNodeInfo]) -> DirTreeModel {
        let mut model = DirTreeModel {
            nodes: Vec::new(),
            roots: Vec::new(),
        };
        model.roots = flatten_level(tree, &mut model.nodes);
        model
    }

    /// The path of each node, indexed by its id.
    fn paths(&self) -> Vec<String> {
        self.nodes.iter().map(|node| node.path.clone()).collect()
    }

    /// The id of each node, keyed by its path.
    fn by_path(&self) -> HashMap<String, NodeId> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(id, node)| (node.path.clone(), id))
            .collect()
    }
}

/// Flattens one level, appending each node (and its descendants) to `flat` and
/// returning this level's node indices.
fn flatten_level(level: &[DirNodeInfo], flat: &mut Vec<FlatDir>) -> Vec<usize> {
    let mut indices = Vec::with_capacity(level.len());
    for node in level {
        let index = flat.len();
        indices.push(index);
        flat.push(FlatDir {
            path: node.path.clone(),
            name: node.name.clone(),
            total_track_count: node.total_track_count,
            has_children: !node.children.is_empty(),
            children: Vec::new(),
        });
        flat[index].children = flatten_level(&node.children, flat);
    }
    indices
}

impl TreeModel for DirTreeModel {
    fn children(&self, parent: Option<NodeId>) -> Vec<TreeNode> {
        let indices: &[usize] = match parent {
            None => &self.roots,
            Some(id) => match self.nodes.get(id) {
                Some(node) => &node.children,
                None => &[],
            },
        };
        indices
            .iter()
            .map(|&index| {
                let node = &self.nodes[index];
                if node.has_children {
                    TreeNode::branch(index, node.label())
                } else {
                    TreeNode::leaf(index, node.label())
                }
            })
            .collect()
    }
}

/// The Folders view's portable controls.
pub struct FoldersView {
    ui: Ui<Msg>,
    tree: TreeView<Msg>,
    include: CheckBox<Msg>,
    table: TrackView,
    /// The path of each tree node, indexed by its id (shared with the tree's
    /// select mapper).
    paths: Rc<RefCell<Vec<String>>>,
    /// The id of each node, keyed by its path, for restoring the selection.
    by_path: RefCell<HashMap<String, NodeId>>,
    /// The library signature the tree was built from (`(scanning, tracks)`).
    applied_library: Cell<Option<(bool, usize)>>,
    /// The include-subfolders state last mirrored into the checkbox.
    applied_include: Cell<bool>,
    /// The selected path last mirrored into the tree.
    applied_selection: RefCell<Option<String>>,
    /// The model revision last mirrored into the table.
    applied_revision: Cell<u64>,
}

impl FoldersView {
    /// Creates the (empty) tree, the include-subfolders checkbox and the track
    /// table.
    pub fn new(ui: &Ui<Msg>) -> FoldersView {
        let paths = Rc::new(RefCell::new(Vec::new()));
        let tree = {
            let paths = Rc::clone(&paths);
            TreeView::with_model(
                ui,
                Rect::default(),
                DirTreeModel {
                    nodes: Vec::new(),
                    roots: Vec::new(),
                },
            )
            .expect("create folder tree")
            .on_select(move |id| paths.borrow().get(id).cloned().map(Msg::FoldersSelect))
        };
        let include = CheckBox::new(ui, Rect::default(), "Include subfolders")
            .expect("create include-subfolders checkbox")
            .on_toggle(|on| Some(Msg::FoldersSubfolders(on)));

        FoldersView {
            ui: ui.clone(),
            tree,
            include,
            table: TrackView::new(ui),
            paths,
            by_path: RefCell::new(HashMap::new()),
            applied_library: Cell::new(None),
            applied_include: Cell::new(false),
            applied_selection: RefCell::new(None),
            applied_revision: Cell::new(u64::MAX),
        }
    }

    /// Moves and sizes the view: the directory tree on the left, the checkbox
    /// above the track table on the right.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let tree_width = dip(TREE_WIDTH).to_px(dpi).value();
        let header = dip(HEADER_HEIGHT).to_px(dpi).value();
        let inset = dip(INSET).to_px(dpi).value();

        let tree_right = (bounds.left + tree_width).min(bounds.right);
        self.ui.apply_moves(&[(
            self.tree.id(),
            Rect::new(bounds.left, bounds.top, tree_right, bounds.bottom),
        )]);

        let right = tree_right + inset;
        self.ui.apply_moves(&[(
            self.include.id(),
            Rect::new(right, bounds.top, bounds.right, bounds.top + header),
        )]);
        self.table.set_bounds(Rect::new(
            right,
            bounds.top + header,
            bounds.right,
            bounds.bottom,
        ));
    }

    /// Shows or hides the whole view (tree, checkbox and table).
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.tree.id(), visible);
        self.ui.set_visible(self.include.id(), visible);
        self.table.set_visible(visible);
    }

    /// Mirrors the model and the library snapshot into the controls.
    ///
    /// The tree is rebuilt when the library changes and not while a scan is
    /// streaming batches (the count changes every batch, which would reinstall
    /// a large model each tick); the first build happens immediately.
    pub fn sync(
        &mut self,
        model: &FoldersModel,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
    ) {
        let scanning = library.is_scanning();
        let signature = (scanning, library.track_count());
        let applied = self.applied_library.get();
        if applied.is_none() || (!scanning && applied != Some(signature)) {
            self.applied_library.set(Some(signature));
            self.install_tree(library.dir_tree());
            // `set_model` clears the selection, so restore it right away.
            self.select_path(model.selected.as_deref());
        }

        if self.applied_include.get() != model.include_subfolders {
            self.applied_include.set(model.include_subfolders);
            self.include.set_checked(model.include_subfolders);
        }

        if *self.applied_selection.borrow() != model.selected {
            *self.applied_selection.borrow_mut() = model.selected.clone();
            self.select_path(model.selected.as_deref());
        }

        if self.applied_revision.get() != model.revision() {
            self.applied_revision.set(model.revision());
            self.set_table_rows(model, library);
        }
        self.table.sync_playing(playing_id);
    }

    /// Rebuilds the table after a header click, preserving the new sort.
    pub fn resort(&mut self, model: &FoldersModel, library: &dyn LibraryDataSource) {
        self.set_table_rows(model, library);
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<Command> {
        self.table.activate(index)
    }

    /// The command to toggle the star of `index`.
    pub fn toggle_star(&self, index: usize) -> Option<Command> {
        self.table.toggle_star(index)
    }

    /// The track at `index`, for a context action.
    pub fn track(&self, index: usize) -> Option<TrackInfo> {
        self.table.track(index).cloned()
    }

    /// Installs a fresh directory model and its id/path maps.
    fn install_tree(&mut self, tree: &[DirNodeInfo]) {
        let model = DirTreeModel::new(tree);
        *self.paths.borrow_mut() = model.paths();
        *self.by_path.borrow_mut() = model.by_path();
        self.tree.set_model(model);
    }

    /// Selects the tree row for `path`, if it is present; clears otherwise.
    fn select_path(&self, path: Option<&str>) {
        let id = path.and_then(|path| self.by_path.borrow().get(path).copied());
        self.tree.select(id);
    }

    /// Rebuilds the track table from the model's current filter and sort.
    fn set_table_rows(&mut self, model: &FoldersModel, library: &dyn LibraryDataSource) {
        let tracks: Vec<&TrackInfo> = library
            .tracks()
            .iter()
            .filter(|track| model.matches(track))
            .collect();
        self.table.set_rows(&tracks, model.table.sort);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str, name: &str, children: Vec<DirNodeInfo>) -> DirNodeInfo {
        DirNodeInfo {
            path: path.to_string(),
            name: name.to_string(),
            direct_track_count: children.len(),
            total_track_count: children.len() + 1,
            children,
        }
    }

    fn sample() -> Vec<DirNodeInfo> {
        vec![
            node(
                r"C:\music",
                "music",
                vec![
                    node(r"C:\music\Album", "Album", Vec::new()),
                    node(r"C:\music\Live", "Live", Vec::new()),
                ],
            ),
            node(r"D:\other", "other", Vec::new()),
        ]
    }

    #[test]
    fn children_are_looked_up_by_id_not_copied_from_a_slice() {
        let model = DirTreeModel::new(&sample());
        let roots = model.children(None);
        assert_eq!(roots.len(), 2, "two root folders");
        assert_eq!(roots[0].label, "music (3)");
        assert!(roots[0].has_children, "music has subfolders");
        assert!(!roots[1].has_children, "other is a leaf");

        // The root's children are reached only through its id, not preloaded.
        let children = model.children(Some(roots[0].id));
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].label, "Album (1)");
        assert_eq!(children[1].label, "Live (1)");
        assert!(
            model.children(Some(roots[1].id)).is_empty(),
            "a leaf has no children"
        );
    }

    #[test]
    fn paths_and_by_path_round_trip_every_node() {
        let model = DirTreeModel::new(&sample());
        let paths = model.paths();
        let by_path = model.by_path();
        assert_eq!(paths.len(), model.nodes.len());
        for (id, path) in paths.iter().enumerate() {
            assert_eq!(by_path.get(path), Some(&id), "id round-trips through path");
        }
    }
}
