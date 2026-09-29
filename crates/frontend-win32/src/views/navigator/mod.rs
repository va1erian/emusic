//! Win32 navigator (#109, #476): a self-drawn list of the shared view sections
//! (`emusic_ui::panels::navigator`) followed by the dynamic PLAYLISTS section,
//! each row an icon (Segoe Fluent Icons) plus a label, with a selection
//! highlight.
//!
//! The sections, the runtime playlists and the click intents come from the
//! model; this view only owns the child window and draws it. A right click
//! raises a context event for the row under the cursor; the app decides which
//! rows actually have a context menu (#242: the Music row's "Shuffle all",
//! #476: a playlist's Play/Shuffle/Rename/Export/Delete). The widget is also a
//! drop target, so dragging tracks onto a playlist row adds them.

mod draw;
mod rows;
mod widget;

use std::cell::Cell;

use emusic_ui::panels::navigator::Navigator;
use emusic_ui::state::View;
use win32ui::prelude::*;
use win32ui::{Control, Custom};

use crate::app::Msg;
use widget::NavigatorWidget;

/// What the navigator raises for the app to act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigatorEvent {
    /// The user clicked this view's row.
    Select(View),
    /// The user right-clicked this view's row, requesting its context menu.
    Context(View),
    /// The user clicked a playlist row.
    SelectPlaylist(u64),
    /// The user right-clicked a playlist row, requesting its context menu.
    ContextPlaylist(u64),
    /// The user clicked the `+` on the playlists heading.
    AddPlaylist,
    /// F2 on a focused playlist row: rename it.
    RenamePlaylist(u64),
    /// Delete on a focused playlist row: delete it.
    DeletePlaylist(u64),
    /// Tracks were dropped onto a playlist row.
    DropTracks { playlist: u64, tracks: Vec<u64> },
}

/// The Win32 navigator: a child window drawing the shared view list.
pub struct NavigatorView {
    custom: Custom<NavigatorWidget, Msg>,
    applied_revision: Cell<u64>,
}

impl NavigatorView {
    /// Creates the navigator and maps its events to [`Msg`].
    pub fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let widget = NavigatorWidget::new(ui.dpi());
        let custom = Custom::new(ui, widget)?
            .on_event(|event| Some(navigator_message(event)))
            .accept_drops()?;
        Ok(Self {
            custom,
            applied_revision: Cell::new(u64::MAX),
        })
    }

    /// Mirrors the model's playlists, highlighted playlist and selection into
    /// the widget, repainting only when the model revision changed.
    pub fn sync(&self, navigator: &Navigator) {
        if self.applied_revision.get() == navigator.revision() {
            return;
        }
        self.applied_revision.set(navigator.revision());
        self.custom.widget().borrow_mut().set_state(
            navigator.playlists(),
            navigator.highlighted_playlist(),
            navigator.selected(),
        );
        self.custom.invalidate();
    }

    /// Rebuilds the row fonts from the current appearance metrics and repaints.
    pub fn apply_appearance(&self, ui: &Ui<Msg>) {
        self.custom
            .widget()
            .borrow_mut()
            .set_metrics(crate::appearance::metrics(), ui.dpi());
        self.custom.invalidate();
    }
}

impl AsControl for NavigatorView {
    fn control(&self) -> &Control {
        self.custom.control()
    }
}

/// Maps a navigator event to the app message that handles it.
fn navigator_message(event: NavigatorEvent) -> Msg {
    match event {
        NavigatorEvent::Select(view) => Msg::Navigate(view),
        NavigatorEvent::Context(view) => Msg::NavigatorContext(view),
        NavigatorEvent::SelectPlaylist(id) => Msg::NavigatePlaylist(id),
        NavigatorEvent::ContextPlaylist(id) => Msg::NavigatorPlaylistContext(id),
        NavigatorEvent::AddPlaylist => Msg::NavigatorAddPlaylist,
        NavigatorEvent::RenamePlaylist(id) => Msg::NavigatorRenamePlaylist(id),
        NavigatorEvent::DeletePlaylist(id) => Msg::NavigatorDeletePlaylist(id),
        NavigatorEvent::DropTracks { playlist, tracks } => {
            Msg::NavigatorDropTracks { playlist, tracks }
        }
    }
}
