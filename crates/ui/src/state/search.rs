//! State for the global search popup (#22): its result entries and the
//! open/query/selection state.

/// One flattened entry in the global search popup's results, in display
/// order across all three sections.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchPopupItem {
    Artist(String),
    Album { name: String, artist: String },
    Track(u64),
}

/// State for the Ctrl+Shift+F / Ctrl+K global search popup (#22): its own
/// query text (independent of the top-bar box), open/closed, and the
/// keyboard-selected row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchPopupState {
    pub open: bool,
    pub query: String,
    /// Index into the flattened, currently-shown result list.
    pub selected: usize,
}

impl SearchPopupState {
    /// Opens the popup, focused on a blank query, ready for typing.
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}
