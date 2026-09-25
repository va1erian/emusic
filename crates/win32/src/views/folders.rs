//! Win32 Folders view (#111): a lazily-loaded directory `TreeView`, an
//! "include subfolders" checkbox and the shared track table for the selected
//! folder.
//!
//! The selection, filter and track table live in `emusic-ui`'s
//! [`FoldersView`](emusic_ui::views::folders::FoldersView); this module only
//! owns the native controls and maps their events to [`Msg`]s.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use emusic_ui::library_api::DirNodeInfo;
use emusic_ui::views::folders::FoldersView as FoldersModel;
use win32ui::prelude::*;
use win32ui::{CheckBox, Control, Layout, Menu, Node, TreeModel, TreeView, column, dip, row};

use crate::app::Msg;
use crate::views::track_table::TrackView;

/// The directory tree panel's width, in design units.
const TREE_WIDTH: f32 = 260.0;

/// One directory, flattened: its display data and its direct children's
/// indices into [`DirTreeModel::nodes`]. Storing child indices (not child
/// slices) means building the model never copies a subtree.
struct FlatNode {
    path: String,
    name: String,
    total_track_count: usize,
    has_children: bool,
    children: Vec<usize>,
}

/// A lazily-loaded [`TreeModel`] over the library's directory grouping.
///
/// The library snapshot is copied once into a flat array (the row strings
/// only); expanding a node then walks a handful of indices. `children` is only
/// called for the roots and for branches the control actually expands, so an
/// unopened branch costs nothing beyond its own entry.
struct DirTreeModel {
    nodes: Vec<FlatNode>,
    roots: Vec<usize>,
    by_path: HashMap<String, usize>,
}

impl DirTreeModel {
    fn new(tree: &[DirNodeInfo]) -> Self {
        let mut nodes = Vec::new();
        let mut by_path = HashMap::new();
        let roots = flatten_level(tree, &mut nodes, &mut by_path);
        Self {
            nodes,
            roots,
            by_path,
        }
    }
}

/// Flattens one level, appending each node (and its descendants) to `flat` and
/// returning the indices of this level's nodes.
fn flatten_level(
    level: &[DirNodeInfo],
    flat: &mut Vec<FlatNode>,
    by_path: &mut HashMap<String, usize>,
) -> Vec<usize> {
    let mut indices = Vec::with_capacity(level.len());
    for node in level {
        let index = flat.len();
        indices.push(index);
        flat.push(FlatNode {
            path: node.path.clone(),
            name: node.name.clone(),
            total_track_count: node.total_track_count,
            has_children: !node.children.is_empty(),
            children: Vec::new(),
        });
        by_path.insert(node.path.clone(), index);
        flat[index].children = flatten_level(&node.children, flat, by_path);
    }
    indices
}

impl TreeModel for DirTreeModel {
    type Key = String;

    fn children(&self, parent: Option<&String>) -> Vec<Node<String>> {
        let indices: &[usize] = match parent {
            None => &self.roots,
            Some(path) => self
                .by_path
                .get(path)
                .map_or(&[], |&index| self.nodes[index].children.as_slice()),
        };
        indices
            .iter()
            .map(|&index| {
                let node = &self.nodes[index];
                let text = format!("{} ({})", node.name, node.total_track_count);
                if node.has_children {
                    Node::branch(node.path.clone(), text)
                } else {
                    Node::leaf(node.path.clone(), text)
                }
            })
            .collect()
    }
}

/// The Folders view's native controls.
pub struct FoldersView {
    tree: TreeView<String, Msg>,
    include: CheckBox<Msg>,
    table: TrackView,
    /// The library snapshot the tree was built from (`(scanning, track count)`),
    /// or `None` before the first build.
    applied_library: Cell<Option<(bool, usize)>>,
    /// The model revision last mirrored into the table.
    applied_revision: Cell<u64>,
    applied_include: Cell<bool>,
    applied_selection: RefCell<Option<String>>,
}

impl FoldersView {
    /// Creates the tree, the checkbox and the track table.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        let tree = TreeView::new(ui, DirTreeModel::new(&[]))?
            .on_select(|path| Some(Msg::FoldersSelect(path.clone())))
            .on_context(|path| Some(Msg::FoldersContext(path.clone())));
        let include = CheckBox::new(ui, "Include subfolders")?
            .on_toggle(|on| Some(Msg::FoldersSubfolders(on)));
        let table = TrackView::new(ui)?;
        Ok(Self {
            tree,
            include,
            table,
            applied_library: Cell::new(None),
            applied_revision: Cell::new(u64::MAX),
            applied_include: Cell::new(false),
            applied_selection: RefCell::new(None),
        })
    }

    /// Mirrors the model and the library into the controls.
    pub fn sync(
        &mut self,
        model: &FoldersModel,
        library: &dyn emusic_ui::library_api::LibraryDataSource,
        playing_id: Option<u64>,
    ) {
        // Reinstall the tree only when it can actually differ and not while a
        // scan is streaming batches (the count changes on every batch, which
        // would reinstall and re-diff a large model each time). The first build
        // happens immediately; a completed scan and any later non-scan change
        // rebuild once.
        let scanning = library.is_scanning();
        let signature = (scanning, library.track_count());
        let applied = self.applied_library.get();
        if applied.is_none() || (!scanning && applied != Some(signature)) {
            self.applied_library.set(Some(signature));
            self.tree.set_model(DirTreeModel::new(library.dir_tree()));
        }

        if self.applied_include.get() != model.include_subfolders {
            self.applied_include.set(model.include_subfolders);
            self.include.set_checked(model.include_subfolders);
        }

        if *self.applied_selection.borrow() != model.selected {
            *self.applied_selection.borrow_mut() = model.selected.clone();
            if let Some(path) = &model.selected {
                self.tree.select(path);
            }
        }

        if self.applied_revision.get() != model.revision() {
            self.applied_revision.set(model.revision());
            self.set_table_rows(model, library);
        }
        self.table.sync_playing(playing_id);
    }

    /// Rebuilds the table after a header click, preserving the new sort.
    pub fn resort(
        &mut self,
        model: &FoldersModel,
        library: &dyn emusic_ui::library_api::LibraryDataSource,
    ) {
        self.set_table_rows(model, library);
    }

    fn set_table_rows(
        &mut self,
        model: &FoldersModel,
        library: &dyn emusic_ui::library_api::LibraryDataSource,
    ) {
        let tracks: Vec<&emusic_ui::library_api::TrackInfo> = library
            .tracks()
            .iter()
            .filter(|track| model.matches(track))
            .collect();
        self.table.set_rows(&tracks, model.table.sort);
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<emusic_ui::state::Command> {
        self.table.activate(index)
    }

    /// The command to toggle the star of `index` (a star-cell click).
    pub fn toggle_star(&self, index: usize) -> Option<emusic_ui::state::Command> {
        self.table.toggle_star(index)
    }

    /// Runs a context action on the row that opened the menu.
    pub fn run_context(
        &self,
        action: crate::views::track_table::ContextAction,
        hwnd: win32ui::Hwnd,
    ) -> Option<emusic_ui::state::Command> {
        self.table.run_context(action, hwnd)
    }

    pub fn set_context_row(&self, row: usize) {
        self.table.set_context_row(row);
    }

    /// The track whose context menu is open, if any.
    pub fn context_track(&self) -> Option<emusic_ui::library_api::TrackInfo> {
        self.table.context_track()
    }

    pub fn context_menu(&self) -> &Menu<Msg> {
        self.table.context_menu()
    }

    /// The context menu shown on a tree node: a scoped shuffle of that folder.
    pub fn shuffle_menu(&self, path: &str) -> Menu<Msg> {
        let path = path.to_string();
        Menu::new().item("Shuffle play", None, move || {
            Msg::FoldersShuffle(path.clone())
        })
    }

    /// Shows or hides the whole view (its tree, checkbox and table).
    pub fn set_visible(&self, visible: bool) {
        self.tree.set_visible(visible);
        self.include.set_visible(visible);
        self.table.set_visible(visible);
    }

    /// The tree on the left, and the checkbox above the track table on the
    /// right.
    pub fn layout(&self) -> Layout {
        row![
            self.tree.width(dip(TREE_WIDTH)),
            column![self.include.layout_item(), self.table.fill(1)].fill(1),
        ]
    }
}

impl AsControl for FoldersView {
    fn control(&self) -> &Control {
        self.tree.control()
    }
}
