//! Global search popup model (#22, #102): the flattened result list, keyboard
//! selection and activation, leaving only drawing to the frontends.
//!
//! Artist/album matching is a plain normalized-substring search over the
//! (small) artist/album lists; the track section reuses the
//! [`SearchEngine`](crate::search::SearchEngine)'s already-computed match set
//! so the popup never re-scans the full library itself.

use emusic_search::normalize_text;

use crate::state::{Command, SearchPopupItem};
use crate::views::{Commands, Ctx};

/// Results shown per section.
pub const SECTION_LIMIT: usize = 5;

/// A user intent on the global search popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchPopupMsg {
    /// Replace the query text.
    SetQuery(String),
    /// Move the selection up by one row (wrapping).
    MoveUp,
    /// Move the selection down by one row (wrapping).
    MoveDown,
    /// Select the row at `index` (e.g. a click).
    Select(usize),
    /// Activate the selected row.
    Activate,
    /// Close the popup.
    Close,
}

/// One flattened, rendered popup row: the item plus its precomputed display
/// label (built in the same pass that finds it, so rendering never has to
/// re-scan the library to look a track back up by id).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchPopupRow {
    pub item: SearchPopupItem,
    pub label: String,
}

impl SearchPopupRow {
    /// The section this row belongs to, for a heading in the list.
    pub fn section(&self) -> &'static str {
        match self.item {
            SearchPopupItem::Artist(_) => "ARTISTS",
            SearchPopupItem::Album { .. } => "ALBUMS",
            SearchPopupItem::Track(_) => "TRACKS",
        }
    }
}

/// The global search popup's model: its state plus the flattened result list.
#[derive(Debug, Default)]
pub struct SearchPopup {
    /// Open/closed, query text and selected row index.
    pub state: crate::state::SearchPopupState,
    /// The flattened, capped result list, rebuilt by
    /// [`SearchPopup::refresh`].
    rows: Vec<SearchPopupRow>,
    /// Every track id the current query matches (not just the capped rows),
    /// used as the play context when a track row is activated (#134).
    matched_track_ids: Vec<u64>,
    /// Bumped whenever the query, open state or selection changes.
    revision: u64,
}

impl SearchPopup {
    /// Rebuilds the result list from the library snapshot, the track search
    /// and the current query, clamping the selection.
    pub fn refresh(&mut self, cx: &Ctx, track_match: impl Fn(u64) -> bool) {
        let (rows, matched) = if self.state.open {
            collect_rows(cx, &self.state.query, &track_match)
        } else {
            (Vec::new(), Vec::new())
        };
        if rows != self.rows {
            self.rows = rows;
            self.revision += 1;
        }
        if matched != self.matched_track_ids {
            self.matched_track_ids = matched;
            self.revision += 1;
        }
        let clamped = if self.rows.is_empty() {
            0
        } else {
            self.state.selected.min(self.rows.len() - 1)
        };
        if clamped != self.state.selected {
            self.state.selected = clamped;
            self.revision += 1;
        }
    }

    /// The flattened result rows.
    pub fn rows(&self) -> &[SearchPopupRow] {
        &self.rows
    }

    /// Whether the popup is open.
    pub fn is_open(&self) -> bool {
        self.state.open
    }

    /// The message shown when there are no rows.
    pub fn empty_message(&self) -> &'static str {
        if self.state.query.is_empty() {
            "Type to search your library."
        } else {
            "No matches."
        }
    }

    /// The revision counter, bumped whenever the displayed state changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies one user intent, queueing any resulting commands.
    ///
    /// Unlike the other view models this doesn't need a [`Ctx`]: its rows and
    /// track match set are already captured by [`SearchPopup::refresh`], so
    /// `update` works from the model alone (which keeps it trivially
    /// testable).
    pub fn update(&mut self, msg: SearchPopupMsg, out: &mut Commands) {
        match msg {
            SearchPopupMsg::SetQuery(query) => {
                if self.state.query != query {
                    self.state.query = query;
                    self.state.selected = 0;
                    self.revision += 1;
                }
            }
            SearchPopupMsg::MoveUp => {
                if !self.rows.is_empty() {
                    self.state.selected =
                        (self.state.selected + self.rows.len() - 1) % self.rows.len();
                    self.revision += 1;
                }
            }
            SearchPopupMsg::MoveDown => {
                if !self.rows.is_empty() {
                    self.state.selected = (self.state.selected + 1) % self.rows.len();
                    self.revision += 1;
                }
            }
            SearchPopupMsg::Select(index) => {
                if index < self.rows.len() {
                    self.state.selected = index;
                    self.revision += 1;
                }
            }
            SearchPopupMsg::Activate => {
                if let Some(row) = self.rows.get(self.state.selected).cloned() {
                    activate(&row.item, &self.matched_track_ids, out);
                }
                self.state.close();
                self.revision += 1;
            }
            SearchPopupMsg::Close => {
                self.state.close();
                self.revision += 1;
            }
        }
    }
}

/// Builds the flattened, capped result list shown in the popup (up to
/// [`SECTION_LIMIT`] artists, then albums, then tracks), plus every track id
/// the query matches for the play context.
fn collect_rows(
    cx: &Ctx,
    query: &str,
    track_match: &impl Fn(u64) -> bool,
) -> (Vec<SearchPopupRow>, Vec<u64>) {
    let Some(library) = cx.library else {
        return (Vec::new(), Vec::new());
    };
    if query.trim().is_empty() {
        return (Vec::new(), Vec::new());
    }
    let needle = normalize_text(query);

    let mut rows: Vec<SearchPopupRow> = library
        .artists()
        .iter()
        .filter(|a| normalize_text(&a.name).contains(&needle))
        .take(SECTION_LIMIT)
        .map(|a| SearchPopupRow {
            item: SearchPopupItem::Artist(a.name.clone()),
            label: a.name.clone(),
        })
        .collect();

    rows.extend(
        library
            .albums()
            .iter()
            .filter(|a| normalize_text(&a.name).contains(&needle))
            .take(SECTION_LIMIT)
            .map(|a| SearchPopupRow {
                item: SearchPopupItem::Album {
                    name: a.name.clone(),
                    artist: a.artist.clone(),
                },
                label: format!("{} — {}", a.name, a.artist),
            }),
    );

    let matched: Vec<u64> = library
        .tracks()
        .iter()
        .filter(|t| track_match(t.id))
        .map(|t| t.id)
        .collect();

    rows.extend(
        library
            .tracks()
            .iter()
            .filter(|t| track_match(t.id))
            .take(SECTION_LIMIT)
            .map(|t| SearchPopupRow {
                item: SearchPopupItem::Track(t.id),
                label: format!("{} — {}", t.title, t.artist),
            }),
    );

    (rows, matched)
}

/// Enter or a click on an item: tracks play directly; artists/albums
/// navigate to their view and seed the top-bar search box with the name so
/// the destination view is filtered down to the picked item.
fn activate(item: &SearchPopupItem, matched_track_ids: &[u64], out: &mut Commands) {
    match item {
        SearchPopupItem::Artist(name) => out.push(Command::GoToArtist(name.clone())),
        SearchPopupItem::Album { name, artist } => out.push(Command::GoToAlbum {
            name: name.clone(),
            artist: artist.clone(),
        }),
        SearchPopupItem::Track(id) => {
            // Context: every track the query matches, not just the section's
            // on-screen top [`SECTION_LIMIT`] (#134) — the same set the Music
            // view would show filtered to this query, so playing a search hit
            // queues up the whole match set rather than a five-track sliver.
            out.play_track(*id, matched_track_ids.iter().copied());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockLibrary;

    fn ctx<'a>(library: &'a dyn crate::library_api::LibraryDataSource) -> Ctx<'a> {
        Ctx::with_library(&[], None, library)
    }

    fn as_source(library: &MockLibrary) -> &dyn crate::library_api::LibraryDataSource {
        library
    }

    #[test]
    fn closed_popup_has_no_rows() {
        let library = MockLibrary::new();
        let mut popup = SearchPopup::default();
        popup.refresh(&ctx(as_source(&library)), |_| true);
        assert!(popup.rows().is_empty());
    }

    #[test]
    fn query_populates_capped_sections() {
        let library = MockLibrary::new();
        let mut popup = SearchPopup::default();
        popup.state.open();
        // A query that matches something in the mock library.
        let artist = as_source(&library).artists()[0].name.clone();
        popup.update(
            SearchPopupMsg::SetQuery(artist.clone()),
            &mut Commands::new(),
        );
        popup.refresh(&ctx(as_source(&library)), |_| true);
        assert!(!popup.rows().is_empty());
        // Every artist row matches; at most SECTION_LIMIT of each section.
        let artists = popup
            .rows()
            .iter()
            .filter(|row| row.section() == "ARTISTS")
            .count();
        assert!(artists <= SECTION_LIMIT);
    }

    #[test]
    fn selection_moves_and_wraps() {
        let library = MockLibrary::new();
        let mut popup = SearchPopup::default();
        popup.state.open();
        popup.update(
            SearchPopupMsg::SetQuery(String::new()),
            &mut Commands::new(),
        );
        popup.refresh(&ctx(as_source(&library)), |_| true); // empty query -> no rows
        popup.update(SearchPopupMsg::MoveDown, &mut Commands::new());
        assert_eq!(popup.state.selected, 0, "no rows, selection stays put");
    }

    #[test]
    fn activating_a_track_plays_with_the_matching_context() {
        let library = MockLibrary::new();
        let track_id = as_source(&library).tracks()[0].id;
        let mut popup = SearchPopup::default();
        popup.state.open();
        // Match everything, so the context is the whole library.
        popup.refresh(&ctx(as_source(&library)), |_| true);
        popup.update(
            SearchPopupMsg::SetQuery("match".to_string()),
            &mut Commands::new(),
        );
        popup.refresh(&ctx(as_source(&library)), |_| true);
        assert!(!popup.rows().is_empty());

        // Select the first track row and activate it.
        let index = popup
            .rows()
            .iter()
            .position(|row| matches!(row.item, SearchPopupItem::Track(_)))
            .expect("a track row exists");
        popup.update(SearchPopupMsg::Select(index), &mut Commands::new());
        let mut out = Commands::new();
        popup.update(SearchPopupMsg::Activate, &mut out);
        assert!(!popup.is_open());
        let commands = out.into_vec();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            Command::PlayTrack { context, .. } => {
                assert_eq!(context.len(), as_source(&library).tracks().len());
                assert!(context.contains(&track_id));
            }
            other => panic!("expected PlayTrack, got {other:?}"),
        }
    }
}
