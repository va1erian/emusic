//! Column-browser view model (#16, #93, #97, #99).
//!
//! [`ColumnBrowser`] owns the persistent state (visibility, splitter height
//! and per-pane selections), the cascading facet lists the three panes show,
//! and the track-matching rule. User intents arrive as
//! [`ColumnBrowserMsg`]; the pane widgets and rendering stay in the
//! frontends. The facet lists are rebuilt from the library snapshot each
//! frame by [`ColumnBrowser::refresh`].

pub mod selection;

use std::collections::{BTreeSet, HashMap};

use crate::library_api::TrackInfo;
use crate::views::Ctx;

pub use selection::PaneSelection;

/// Default splitter height, in pixels (the pane strip above the table),
/// sized so roughly eight to ten rows are visible in each pane.
pub const DEFAULT_HEIGHT: f32 = 200.0;
/// Smallest the column browser may be dragged to.
pub const MIN_HEIGHT: f32 = 80.0;
/// Largest the column browser may be dragged to.
pub const MAX_HEIGHT: f32 = 480.0;

/// Which of the three cascading panes a message targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Genre,
    Artist,
    Album,
}

/// A user intent on the column browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnBrowserMsg {
    /// A pane row was clicked; `value` is `None` for the "All" row.
    RowClicked {
        pane: Pane,
        value: Option<String>,
        ctrl: bool,
    },
}

/// One selectable row in a pane; `value == None` is the leading "All (N)"
/// row, and an empty value is shown as "(unknown)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetEntry {
    pub value: Option<String>,
    pub count: usize,
}

impl FacetEntry {
    /// Text shown for this row, without the count.
    pub fn label(&self) -> &str {
        match self.value.as_deref() {
            None => "All",
            Some("") => "(unknown)",
            Some(value) => value,
        }
    }
}

/// The rows shown by the three panes this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnBrowserFacets {
    pub genres: Vec<FacetEntry>,
    pub artists: Vec<FacetEntry>,
    pub albums: Vec<FacetEntry>,
}

impl ColumnBrowserFacets {
    /// The rows of `pane`.
    pub fn pane(&self, pane: Pane) -> &[FacetEntry] {
        match pane {
            Pane::Genre => &self.genres,
            Pane::Artist => &self.artists,
            Pane::Album => &self.albums,
        }
    }
}

/// Persistent column-browser state: visibility, splitter height, the
/// selection of each pane, and the cascading facet lists built from the
/// library snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnBrowser {
    pub visible: bool,
    pub height: f32,
    pub genres: PaneSelection,
    pub artists: PaneSelection,
    pub albums: PaneSelection,
    /// Facet rows shown by each pane, rebuilt by [`ColumnBrowser::refresh`].
    facets: ColumnBrowserFacets,
    /// Bumped whenever the facet lists or selections change.
    revision: u64,
}

impl Default for ColumnBrowser {
    fn default() -> Self {
        Self {
            visible: true,
            height: DEFAULT_HEIGHT,
            genres: PaneSelection::default(),
            artists: PaneSelection::default(),
            albums: PaneSelection::default(),
            facets: ColumnBrowserFacets {
                genres: Vec::new(),
                artists: Vec::new(),
                albums: Vec::new(),
            },
            revision: 0,
        }
    }
}

impl ColumnBrowser {
    /// Whether `track` passes all three panes' filters.
    pub fn matches(&self, track: &TrackInfo) -> bool {
        self.genres.matches(&track.genre)
            && self.artists.matches(&track.artist)
            && self.albums.matches(&track.album)
    }

    /// Rebuilds the cascading facet lists from the library snapshot,
    /// pruning child selections the parent no longer offers, and bumping
    /// [`ColumnBrowser::revision`] when what the panes show changed.
    pub fn refresh(&mut self, cx: &Ctx) {
        let genres = entries_for(cx.tracks, |t| &t.genre);
        self.genres.retain(&values(&genres));

        let by_genre: Vec<&TrackInfo> = cx
            .tracks
            .iter()
            .copied()
            .filter(|t| self.genres.matches(&t.genre))
            .collect();
        let artists = entries_for(&by_genre, |t| &t.artist);
        self.artists.retain(&values(&artists));

        let by_artist: Vec<&TrackInfo> = by_genre
            .into_iter()
            .filter(|t| self.artists.matches(&t.artist))
            .collect();
        let albums = entries_for(&by_artist, |t| &t.album);
        self.albums.retain(&values(&albums));

        let facets = ColumnBrowserFacets {
            genres,
            artists,
            albums,
        };
        if facets != self.facets {
            self.facets = facets;
            self.revision += 1;
        }
    }

    /// The rows shown by the three panes this frame.
    pub fn facets(&self) -> &ColumnBrowserFacets {
        &self.facets
    }

    /// The revision counter, bumped whenever the panes' content changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Replaces the facet lists wholesale, marking the revision changed.
    /// Used to seed a fresh model in tests and by retained-mode frontends
    /// that build the lists themselves.
    pub fn set_facets(&mut self, facets: ColumnBrowserFacets) {
        if facets != self.facets {
            self.facets = facets;
            self.revision += 1;
        }
    }

    /// Applies one user intent to the model.
    pub fn update(&mut self, msg: ColumnBrowserMsg) {
        match msg {
            ColumnBrowserMsg::RowClicked { pane, value, ctrl } => {
                let selection = match pane {
                    Pane::Genre => &mut self.genres,
                    Pane::Artist => &mut self.artists,
                    Pane::Album => &mut self.albums,
                };
                selection.click(value.as_deref(), ctrl);
                self.revision += 1;
            }
        }
    }
}

/// Builds a pane's rows: "All (N)" first, then each distinct facet sorted
/// case-insensitively, with the number of tracks it represents.
pub fn entries_for(tracks: &[&TrackInfo], key: impl Fn(&TrackInfo) -> &str) -> Vec<FacetEntry> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for track in tracks.iter().copied() {
        *counts.entry(key(track).to_string()).or_default() += 1;
    }

    let mut facets: Vec<(String, usize)> = counts.into_iter().collect();
    facets.sort_by(|a, b| {
        a.0.to_lowercase()
            .cmp(&b.0.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut entries = Vec::with_capacity(facets.len() + 1);
    entries.push(FacetEntry {
        value: None,
        count: tracks.len(),
    });
    entries.extend(facets.into_iter().map(|(value, count)| FacetEntry {
        value: Some(value),
        count,
    }));
    entries
}

/// The distinct facet values a pane offers (excluding the "All" row).
fn values(entries: &[FacetEntry]) -> BTreeSet<String> {
    entries.iter().filter_map(|e| e.value.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(genre: &str, artist: &str, album: &str) -> TrackInfo {
        TrackInfo {
            genre: genre.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            ..TrackInfo::default()
        }
    }

    fn refs(tracks: &[TrackInfo]) -> Vec<&TrackInfo> {
        tracks.iter().collect()
    }

    #[test]
    fn entries_start_with_all_and_count_each_facet() {
        let a = track("Rock", "A", "First");
        let b = track("Rock", "B", "Second");
        let c = track("Jazz", "A", "First");
        let tracks = [&a, &b, &c];

        let entries = entries_for(&tracks, |t| &t.genre);
        assert_eq!(entries[0].value, None);
        assert_eq!(entries[0].label(), "All");
        assert_eq!(entries[0].count, 3);

        let genres: Vec<_> = entries[1..].iter().map(|e| e.label()).collect();
        assert_eq!(genres, ["Jazz", "Rock"]);
        assert_eq!(entries[1].count, 1);
        assert_eq!(entries[2].count, 2);
    }

    #[test]
    fn entries_are_sorted_case_insensitively_and_show_unknown() {
        let a = track("Rock", "banana", "X");
        let b = track("Rock", "Apple", "X");
        let c = track("Rock", "", "X");
        let tracks = [&a, &b, &c];

        let entries = entries_for(&tracks, |t| &t.artist);
        let artists: Vec<_> = entries[1..].iter().map(|e| e.label()).collect();
        assert_eq!(artists, ["(unknown)", "Apple", "banana"]);
    }

    #[test]
    fn matches_cascades_across_all_three_panes() {
        let mut browser = ColumnBrowser::default();
        assert!(browser.matches(&track("Rock", "A", "First")));

        browser.update(ColumnBrowserMsg::RowClicked {
            pane: Pane::Genre,
            value: Some("Rock".to_string()),
            ctrl: false,
        });
        assert!(browser.matches(&track("Rock", "A", "First")));
        assert!(!browser.matches(&track("Jazz", "A", "First")));

        browser.update(ColumnBrowserMsg::RowClicked {
            pane: Pane::Artist,
            value: Some("A".to_string()),
            ctrl: false,
        });
        assert!(browser.matches(&track("Rock", "A", "First")));
        assert!(!browser.matches(&track("Rock", "B", "First")));

        browser.update(ColumnBrowserMsg::RowClicked {
            pane: Pane::Album,
            value: Some("First".to_string()),
            ctrl: false,
        });
        assert!(browser.matches(&track("Rock", "A", "First")));
        assert!(!browser.matches(&track("Rock", "A", "Second")));
    }

    #[test]
    fn refresh_cascades_and_prunes_child_selections() {
        let tracks = [
            track("Rock", "A", "First"),
            track("Rock", "B", "Second"),
            track("Jazz", "A", "Third"),
        ];
        let tracks = refs(&tracks);
        let cx = Ctx::new(&tracks, None);
        let mut browser = ColumnBrowser::default();
        browser.refresh(&cx);

        // Restrict to Jazz: the artist pane must only offer A, and a stale
        // "B" selection must be pruned.
        browser.update(ColumnBrowserMsg::RowClicked {
            pane: Pane::Genre,
            value: Some("Jazz".to_string()),
            ctrl: false,
        });
        browser.update(ColumnBrowserMsg::RowClicked {
            pane: Pane::Artist,
            value: Some("B".to_string()),
            ctrl: false,
        });
        browser.refresh(&cx);

        let artists: Vec<_> = browser.facets().artists.iter().map(|e| e.label()).collect();
        assert_eq!(artists, ["All", "A"]);
        assert!(browser.artists.is_all(), "stale artist selection is pruned");
    }

    #[test]
    fn refresh_bumps_revision_when_facets_change() {
        let tracks = [track("Rock", "A", "First")];
        let tracks = refs(&tracks);
        let cx = Ctx::new(&tracks, None);
        let mut browser = ColumnBrowser::default();
        let before = browser.revision();
        browser.refresh(&cx);
        assert!(browser.revision() > before);
        let after = browser.revision();
        browser.refresh(&cx);
        assert_eq!(browser.revision(), after, "stable facets don't bump");
    }
}
