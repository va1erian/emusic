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

/// A lazily-loaded [`TreeModel`] over the library's directory grouping: only
/// the roots are read up front, and a node's children only when it is expanded.
struct DirTreeModel {
    roots: Vec<DirNodeInfo>,
    by_path: HashMap<String, Vec<DirNodeInfo>>,
}

impl DirTreeModel {
    fn new(tree: &[DirNodeInfo]) -> Self {
        let mut by_path = HashMap::new();
        index(tree, &mut by_path);
        Self {
            roots: tree.to_vec(),
            by_path,
        }
    }
}

/// Records every node's children by path, so `children` is a map lookup.
fn index(nodes: &[DirNodeInfo], map: &mut HashMap<String, Vec<DirNodeInfo>>) {
    for node in nodes {
        map.insert(node.path.clone(), node.children.clone());
        index(&node.children, map);
    }
}

/// The row text, matching the egui tree's `"name (total)"`.
fn node_text(node: &DirNodeInfo) -> String {
    format!("{} ({})", node.name, node.total_track_count)
}

impl TreeModel for DirTreeModel {
    type Key = String;

    fn children(&self, parent: Option<&String>) -> Vec<Node<String>> {
        let nodes: &[DirNodeInfo] = match parent {
            None => &self.roots,
            Some(path) => self.by_path.get(path).map_or(&[], Vec::as_slice),
        };
        nodes
            .iter()
            .map(|node| {
                if node.children.is_empty() {
                    Node::leaf(node.path.clone(), node_text(node))
                } else {
                    Node::branch(node.path.clone(), node_text(node))
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
    /// The library snapshot the tree was built from (`(track count, scanning)`),
    /// so it is only rebuilt when the directory grouping changed.
    applied_library: Cell<(usize, bool)>,
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
            applied_library: Cell::new((usize::MAX, true)),
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
        changed: emusic_ui::shell::Changes,
    ) {
        let library_signature = (library.track_count(), library.is_scanning());
        let rebuild_tree = self.applied_library.get() != library_signature
            && (changed.intersects(emusic_ui::shell::Changes::LIBRARY)
                || self.applied_library.get().0 == usize::MAX);
        if rebuild_tree {
            self.applied_library.set(library_signature);
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
