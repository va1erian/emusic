//! Track-table view model (#15, #93, #97, #98): the toolkit-agnostic state
//! and logic behind every embedded track list.
//!
//! [`TrackTable`] owns the persistent sort/selection (and the open Properties
//! dialog) plus the display order derived from the parent's visible tracks.
//! User intents arrive as [`TrackTableMsg`]; [`TrackTable::update`] applies
//! them and queues any [`Command`]s into a [`Commands`] sink. Rendering
//! (cells, stars, playing marker) stays in the frontends; this module holds
//! the column identities/widths/text, the sort order built from header
//! clicks, keyboard navigation and the persistent per-instance state.

pub mod columns;
pub mod selection;
pub mod sort;

use std::collections::HashSet;

use crate::library_api::TrackInfo;
use crate::state::Command;
use crate::views::{Commands, Ctx};

use columns::ColumnId;
use selection::{ClickModifiers, SelectionState};
use sort::SortState;

/// A keyboard navigation intent, decoupled from any toolkit's key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyNav {
    pub key: NavKey,
    /// Extend the selection to the new focus (Shift+arrow).
    pub shift: bool,
    /// Move the focus without changing the selection (Ctrl+arrow).
    pub ctrl: bool,
}

impl KeyNav {
    /// A plain (unmodified) navigation key.
    pub fn new(key: NavKey) -> Self {
        Self {
            key,
            shift: false,
            ctrl: false,
        }
    }
}

/// The navigation key itself, independent of modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    Up,
    Down,
    /// Move up by `rows` visible rows (a page).
    PageUp(usize),
    /// Move down by `rows` visible rows (a page).
    PageDown(usize),
    Home,
    End,
}

/// What a row's context menu asks for. Clipboard/Explorer actions stay in the
/// frontends, since they need no shared state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Play,
    PlayNext,
    AddToQueue,
    ToggleStar,
    EditTags,
    Properties,
}

/// A user intent on the track table, mapped from a frontend's events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackTableMsg {
    /// A column header was clicked; sort by it (or flip direction).
    HeaderClicked(ColumnId),
    /// A row was clicked; `row` is its index in the current display order.
    RowClicked { row: usize, mods: ClickModifiers },
    /// A row was double-clicked or activated with Enter.
    RowActivated(usize),
    /// A navigation key was pressed.
    Nav(KeyNav),
    /// A context-menu entry was chosen for a row.
    Context { row: usize, action: ContextAction },
}

/// Read-only display data for one row, so both frontends format identically.
#[derive(Debug, Clone, Copy)]
pub struct RowView<'a> {
    /// Position in the current display order (0-based; the `#` column shows
    /// `index + 1`).
    pub index: usize,
    /// The track at this position.
    pub track: &'a TrackInfo,
    /// Whether this row is the currently playing track.
    pub is_playing: bool,
}

/// Persistent per-instance state (sort + selection) plus the display order
/// derived from the parent's visible tracks. Each embedding view owns one
/// across frames.
#[derive(Debug, Default)]
pub struct TrackTable {
    pub sort: SortState,
    pub selection: SelectionState,
    /// Indices into the parent's visible slice, in display order, rebuilt by
    /// [`TrackTable::refresh`].
    order: Vec<usize>,
    /// Bumped whenever what the view displays changes (#98), so a
    /// retained-mode frontend can update only the affected controls.
    revision: u64,
    /// The track whose Properties dialog is open, if any. Owned here (rather
    /// than by the shell) so each embedding table gets its own dialog; the
    /// dialog is rendered by the frontend's table widget itself.
    pub properties: Option<TrackInfo>,
}

impl TrackTable {
    /// Rebuilds the display order from `cx` and the current sort, and prunes
    /// the selection to tracks that are still visible. Cheap enough to call
    /// once per frame; bumps [`TrackTable::revision`] when the order or
    /// selection actually changed.
    pub fn refresh(&mut self, cx: &Ctx) {
        let order = sort::sorted_indices(cx.tracks, self.sort);
        let mut changed = order != self.order;
        self.order = order;

        let visible: HashSet<u64> = cx.tracks.iter().map(|track| track.id).collect();
        let before = self.selection.len();
        self.selection.retain_existing(&visible);
        changed |= self.selection.len() != before;

        if changed {
            self.revision += 1;
        }
    }

    /// Applies one user intent, queueing any resulting commands.
    pub fn update(&mut self, msg: TrackTableMsg, cx: &Ctx, out: &mut Commands) {
        match msg {
            TrackTableMsg::HeaderClicked(column) => {
                self.sort.toggle(column);
                self.refresh(cx);
                self.revision += 1;
            }
            TrackTableMsg::RowClicked { row, mods } => {
                let Some(id) = self.track_at(row, cx).map(|track| track.id) else {
                    return;
                };
                let order_ids = self.order_ids(cx);
                self.selection.click(&order_ids, row, id, mods);
                self.revision += 1;
            }
            TrackTableMsg::RowActivated(row) => {
                let Some(id) = self.track_at(row, cx).map(|track| track.id) else {
                    return;
                };
                let context = self.order_ids(cx);
                out.play_track(id, context);
            }
            TrackTableMsg::Nav(nav) => self.navigate(nav, cx),
            TrackTableMsg::Context { row, action } => {
                let Some(track) = self.track_at(row, cx).cloned() else {
                    return;
                };
                match action {
                    ContextAction::Play => {
                        let context = self.order_ids(cx);
                        out.play_track(track.id, context);
                    }
                    ContextAction::PlayNext => out.push(Command::PlayTrackNext(track.id)),
                    ContextAction::AddToQueue => out.push(Command::QueueTrack(track.id)),
                    ContextAction::ToggleStar => out.push(Command::ToggleStarred(track.id)),
                    ContextAction::EditTags => out.push(Command::OpenTagEditor(track.id)),
                    ContextAction::Properties => self.properties = Some(track),
                }
            }
        }
    }

    /// The number of rows currently displayed.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Whether no rows are currently displayed.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// The display data for row `i`, or `None` when it is out of range.
    pub fn row<'a>(&self, i: usize, cx: &Ctx<'a>) -> Option<RowView<'a>> {
        let track = self.track_at(i, cx)?;
        Some(RowView {
            index: i,
            track,
            is_playing: cx.playing_id == Some(track.id),
        })
    }

    /// The visible track ids in display order, for a play context (#134).
    pub fn order_ids(&self, cx: &Ctx) -> Vec<u64> {
        self.order
            .iter()
            .filter_map(|&index| cx.tracks.get(index).map(|track| track.id))
            .collect()
    }

    /// The revision counter, bumped whenever the displayed state changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn track_at<'a>(&self, i: usize, cx: &Ctx<'a>) -> Option<&'a TrackInfo> {
        self.order
            .get(i)
            .and_then(|&index| cx.tracks.get(index))
            .copied()
    }

    fn navigate(&mut self, nav: KeyNav, cx: &Ctx) {
        let len = self.order.len();
        if len == 0 {
            return;
        }
        let current = self.selection.focus.unwrap_or(0);
        let target = match nav.key {
            NavKey::Up => current.saturating_sub(1),
            NavKey::Down => (current + 1).min(len - 1),
            NavKey::PageUp(rows) => current.saturating_sub(rows.max(1)),
            NavKey::PageDown(rows) => (current + rows.max(1)).min(len - 1),
            NavKey::Home => 0,
            NavKey::End => len - 1,
        };
        let order_ids = self.order_ids(cx);
        self.selection
            .navigate(&order_ids, target, nav.shift, nav.ctrl);
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: u64, title: &str, artist: &str) -> TrackInfo {
        TrackInfo {
            id,
            title: title.to_string(),
            artist: artist.to_string(),
            ..TrackInfo::default()
        }
    }

    /// Two tracks in library order (`id` ascending).
    fn two() -> [TrackInfo; 2] {
        [track(1, "Beta", "b"), track(2, "Alpha", "a")]
    }

    #[test]
    fn refresh_orders_by_library_order_without_a_sort() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);
        assert_eq!(table.order_ids(&cx), vec![1, 2]);
        assert_eq!(table.len(), 2);
        assert_eq!(table.row(1, &cx).map(|row| row.track.id), Some(2));
    }

    #[test]
    fn header_click_sorts_and_clicking_again_flips() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);

        table.update(
            TrackTableMsg::HeaderClicked(ColumnId::Title),
            &cx,
            &mut Commands::new(),
        );
        assert_eq!(table.order_ids(&cx), vec![2, 1], "Alpha sorts before Beta");

        table.update(
            TrackTableMsg::HeaderClicked(ColumnId::Title),
            &cx,
            &mut Commands::new(),
        );
        assert_eq!(table.order_ids(&cx), vec![1, 2], "reversed");
    }

    #[test]
    fn row_click_selects_by_track_id() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);

        table.update(
            TrackTableMsg::RowClicked {
                row: 1,
                mods: ClickModifiers::default(),
            },
            &cx,
            &mut Commands::new(),
        );
        assert!(table.selection.is_selected(2));
        assert_eq!(table.selection.focus, Some(1));
    }

    #[test]
    fn row_activated_plays_with_the_visible_order_as_context() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);

        let mut out = Commands::new();
        table.update(TrackTableMsg::RowActivated(1), &cx, &mut out);
        assert_eq!(out.into_vec(), vec![Command::play_track(2, vec![1, 2])]);
    }

    #[test]
    fn nav_moves_focus_and_replaces_the_selection() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);

        table.update(
            TrackTableMsg::Nav(KeyNav::new(NavKey::Down)),
            &cx,
            &mut Commands::new(),
        );
        assert_eq!(table.selection.focus, Some(1));
        assert!(table.selection.is_selected(2));

        table.update(
            TrackTableMsg::Nav(KeyNav::new(NavKey::Up)),
            &cx,
            &mut Commands::new(),
        );
        assert_eq!(table.selection.focus, Some(0));
        assert!(table.selection.is_selected(1));
        assert!(!table.selection.is_selected(2));
    }

    #[test]
    fn nav_home_and_end_jump_to_the_ends() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);

        table.update(
            TrackTableMsg::Nav(KeyNav::new(NavKey::End)),
            &cx,
            &mut Commands::new(),
        );
        assert_eq!(table.selection.focus, Some(1));
        table.update(
            TrackTableMsg::Nav(KeyNav::new(NavKey::Home)),
            &cx,
            &mut Commands::new(),
        );
        assert_eq!(table.selection.focus, Some(0));
    }

    #[test]
    fn shift_nav_extends_the_selection_range() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);

        table.update(
            TrackTableMsg::RowClicked {
                row: 0,
                mods: ClickModifiers::default(),
            },
            &cx,
            &mut Commands::new(),
        );
        table.update(
            TrackTableMsg::Nav(KeyNav {
                key: NavKey::Down,
                shift: true,
                ctrl: false,
            }),
            &cx,
            &mut Commands::new(),
        );
        assert_eq!(table.selection.len(), 2);
        assert!(table.selection.is_selected(1) && table.selection.is_selected(2));
    }

    #[test]
    fn context_actions_emit_commands() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);

        let mut out = Commands::new();
        table.update(
            TrackTableMsg::Context {
                row: 0,
                action: ContextAction::ToggleStar,
            },
            &cx,
            &mut out,
        );
        table.update(
            TrackTableMsg::Context {
                row: 0,
                action: ContextAction::PlayNext,
            },
            &cx,
            &mut out,
        );
        assert_eq!(
            out.into_vec(),
            vec![Command::ToggleStarred(1), Command::PlayTrackNext(1)]
        );
    }

    #[test]
    fn context_properties_opens_the_dialog_without_a_command() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        table.refresh(&cx);

        let mut out = Commands::new();
        table.update(
            TrackTableMsg::Context {
                row: 1,
                action: ContextAction::Properties,
            },
            &cx,
            &mut out,
        );
        assert!(out.is_empty());
        assert_eq!(table.properties.as_ref().map(|track| track.id), Some(2));
    }

    #[test]
    fn refresh_prunes_selection_to_visible_tracks() {
        let tracks = two();
        let all: Vec<&TrackInfo> = tracks.iter().collect();
        let mut table = TrackTable::default();
        table.refresh(&Ctx::new(&all, None));
        table.update(
            TrackTableMsg::RowClicked {
                row: 0,
                mods: ClickModifiers::default(),
            },
            &Ctx::new(&all, None),
            &mut Commands::new(),
        );
        assert!(table.selection.is_selected(1));

        let only_second = [&tracks[1]];
        table.refresh(&Ctx::new(&only_second, None));
        assert!(table.selection.is_empty());
        assert_eq!(table.order_ids(&Ctx::new(&only_second, None)), vec![2]);
    }

    #[test]
    fn revision_advances_when_displayed_state_changes() {
        let tracks = two();
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut table = TrackTable::default();
        let before = table.revision();
        table.refresh(&cx);
        assert!(table.revision() > before);
    }
}
