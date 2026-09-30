//! Artists-view model (#104): the name-sorted artist list with its album and
//! track counts, plus the per-row "Shuffle play" intent. Rendering (the table
//! and its context menu) stays in the app.
//!
//! [`ArtistsView::refresh`] rebuilds the rows only when the library's
//! [`LibraryDataSource::revision`] counter changes, so an unchanged library
//! does no allocation or sorting per frame. User intents arrive as
//! [`ArtistsMsg`].

use std::collections::HashSet;

use crate::library_api::{ArtistInfo, LibraryDataSource};
use crate::search::SearchEngine;
use crate::state::Command;
use crate::views::filter;
use crate::views::{Commands, Ctx};

/// A user intent on the Artists view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtistsMsg {
    /// Start a shuffled playback of that artist's tracks.
    Shuffle(String),
}

/// Persistent Artists-view state plus the name-sorted rows rebuilt from the
/// library snapshot when it changes.
#[derive(Debug, Default)]
pub struct ArtistsView {
    /// Artists in display order, rebuilt by [`ArtistsView::refresh`].
    rows: Vec<ArtistInfo>,
    /// The library revision the rows were built from; `None` when the backend
    /// provides no cheap signal or the rows are stale.
    source_revision: Option<u64>,
    /// The search revision the rows were last filtered for.
    source_search: u64,
    /// Bumped whenever what the view displays changes.
    revision: u64,
}

impl ArtistsView {
    /// Rebuilds the name-sorted rows from the library snapshot in `cx`,
    /// keeping only artists whose name matches the active search or that have a
    /// matching track.
    pub fn refresh(&mut self, cx: &Ctx) {
        let Some(library) = cx.library else {
            return;
        };
        let library_changed = match library.revision() {
            Some(revision) => self.source_revision != Some(revision),
            None => true,
        };
        let search_revision = cx.search_revision();
        let search_changed = self.source_search != search_revision;
        if !library_changed && !search_changed {
            return;
        }
        self.source_revision = library.revision();
        self.source_search = search_revision;

        let mut rows = library.artists().to_vec();
        if let Some(search) = cx.search.filter(|search| search.is_active()) {
            let matching = matching_artists(library, search);
            rows.retain(|artist| {
                filter::name_kept(Some(search), &artist.name)
                    || matching.contains(&artist.name.to_lowercase())
            });
        }
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        if rows != self.rows {
            self.rows = rows;
            self.revision += 1;
        }
    }

    /// The artist rows in display order.
    pub fn rows(&self) -> &[ArtistInfo] {
        &self.rows
    }

    /// The number of artists shown.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no artists are shown.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The header count text, e.g. `"12 artists"`.
    pub fn count_label(&self) -> String {
        format!("{} artists", self.rows.len())
    }

    /// The revision counter, bumped whenever the displayed rows change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies one user intent, queueing any resulting commands. `cx` must
    /// carry the library snapshot ([`Ctx::with_library`]).
    pub fn update(&mut self, msg: ArtistsMsg, cx: &Ctx, out: &mut Commands) {
        match msg {
            ArtistsMsg::Shuffle(name) => {
                let Some(library) = cx.library else {
                    return;
                };
                let ids: Vec<u64> = library
                    .tracks()
                    .iter()
                    .filter(|track| track.artist.eq_ignore_ascii_case(&name))
                    .map(|track| track.id)
                    .collect();
                if !ids.is_empty() {
                    out.push(Command::ShuffleScope {
                        ids,
                        label: format!("Artist — {name}"),
                    });
                }
            }
        }
    }
}

/// The lower-cased artist names that have at least one track matching
/// `search`, so an artist row survives when one of its tracks does.
fn matching_artists(library: &dyn LibraryDataSource, search: &SearchEngine) -> HashSet<String> {
    library
        .tracks()
        .iter()
        .filter(|track| search.is_match(track.id))
        .map(|track| track.artist.to_lowercase())
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_api::LibraryDataSource;
    use crate::mock::MockLibrary;
    use crate::views::test_library::RiggedLibrary;

    fn view_with_library(library: &dyn LibraryDataSource) -> ArtistsView {
        let mut view = ArtistsView::default();
        view.refresh(&Ctx::with_library(&[], None, library));
        view
    }

    /// Ticks `engine` until it stops being pending, polling like the UI does.
    fn settle(engine: &mut SearchEngine, tracks: &[crate::library_api::TrackInfo], query: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            engine.tick(tracks, query);
            if !engine.is_pending() || std::time::Instant::now() > deadline {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// A library with artists "Alpha"/"Beta"/"Gamma" and a track by each.
    fn artist_library() -> RiggedLibrary {
        let mut library = RiggedLibrary::new(1);
        library.artists = vec![
            RiggedLibrary::artist("Alpha"),
            RiggedLibrary::artist("Beta"),
            RiggedLibrary::artist("Gamma"),
        ];
        library.tracks = vec![
            crate::library_api::TrackInfo {
                id: 1,
                title: "One".to_string(),
                artist: "Alpha".to_string(),
                ..crate::library_api::TrackInfo::default()
            },
            crate::library_api::TrackInfo {
                id: 2,
                title: "Two".to_string(),
                artist: "Beta".to_string(),
                ..crate::library_api::TrackInfo::default()
            },
        ];
        library
    }

    #[test]
    fn refresh_sorts_artists_by_name_and_counts_them() {
        let library = MockLibrary::new();
        let view = view_with_library(&library);

        let names: Vec<&str> = view.rows().iter().map(|a| a.name.as_str()).collect();
        let mut expected = names.clone();
        expected.sort();
        assert_eq!(names, expected);
        assert_eq!(view.len(), library.artists().len());
        assert_eq!(
            view.count_label(),
            format!("{} artists", library.artists().len())
        );
    }

    #[test]
    fn refresh_rebuilds_only_on_a_revision_change() {
        let mut library = RiggedLibrary::new(1);
        library.artists = vec![
            RiggedLibrary::artist("Beta"),
            RiggedLibrary::artist("Alpha"),
        ];

        let mut view = ArtistsView::default();
        {
            let cx = Ctx::with_library(&[], None, &library);
            view.refresh(&cx);
            assert_eq!(view.len(), 2);
            assert_eq!(view.rows()[0].name, "Alpha", "rows are sorted");

            let reads = library.artist_reads.get();
            let revision = view.revision();
            view.refresh(&cx);
            assert_eq!(
                library.artist_reads.get(),
                reads,
                "an unchanged revision re-reads nothing"
            );
            assert_eq!(
                view.revision(),
                revision,
                "an unchanged revision doesn't bump"
            );
        }

        library.artists.push(RiggedLibrary::artist("Gamma"));
        library.revision = Some(2);
        let before = view.revision();
        view.refresh(&Ctx::with_library(&[], None, &library));
        assert_eq!(view.len(), 3, "a changed revision rebuilds");
        assert_eq!(view.rows()[2].name, "Gamma");
        assert!(
            view.revision() > before,
            "a changed list bumps the revision"
        );
    }

    #[test]
    fn shuffle_resolves_the_artists_track_ids() {
        let library = MockLibrary::new();
        let mut view = view_with_library(&library);
        let name = view.rows()[0].name.clone();
        let expected: Vec<u64> = library
            .tracks()
            .iter()
            .filter(|track| track.artist.eq_ignore_ascii_case(&name))
            .map(|track| track.id)
            .collect();

        let mut out = Commands::new();
        view.update(
            ArtistsMsg::Shuffle(name.clone()),
            &Ctx::with_library(&[], None, &library),
            &mut out,
        );
        assert_eq!(
            out.into_vec(),
            vec![Command::ShuffleScope {
                ids: expected,
                label: format!("Artist — {name}"),
            }]
        );
    }

    #[test]
    fn shuffle_without_matching_tracks_is_a_no_op() {
        let library = MockLibrary::new();
        let mut view = view_with_library(&library);
        let mut out = Commands::new();
        view.update(
            ArtistsMsg::Shuffle("No Such Artist".to_string()),
            &Ctx::with_library(&[], None, &library),
            &mut out,
        );
        assert!(out.is_empty(), "an empty scope is never queued");
    }

    #[test]
    fn search_keeps_artists_by_name_or_matching_track_and_restores_on_clear() {
        let library = artist_library();
        let mut engine = SearchEngine::new();
        settle(&mut engine, &library.tracks, "one");

        let mut view = ArtistsView::default();
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        let names: Vec<&str> = view.rows().iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Alpha"], "only the artist of the matching track");

        settle(&mut engine, &library.tracks, "beta");
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        let names: Vec<&str> = view.rows().iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Beta"], "the artist name matches too");

        settle(&mut engine, &library.tracks, "");
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        assert_eq!(view.len(), 3, "clearing the query restores every artist");
    }

    #[test]
    fn a_query_matching_no_artist_or_track_yields_no_rows() {
        let library = artist_library();
        let mut engine = SearchEngine::new();
        settle(&mut engine, &library.tracks, "zzzz");

        let mut view = ArtistsView::default();
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        assert!(view.is_empty());
    }
}
