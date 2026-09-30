//! Genres-view model (#104): the name-sorted genre list with its track
//! counts, plus the per-row "Shuffle play" intent. Rendering (the table and
//! its context menu) stays in the app.
//!
//! [`GenresView::refresh`] rebuilds the rows only when the library's
//! [`LibraryDataSource::revision`] counter changes, so an unchanged library
//! does no allocation or sorting per frame. User intents arrive as
//! [`GenresMsg`].

use std::collections::HashSet;

use crate::library_api::{GenreInfo, LibraryDataSource};
use crate::search::SearchEngine;
use crate::state::Command;
use crate::views::filter;
use crate::views::{Commands, Ctx};

/// A user intent on the Genres view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenresMsg {
    /// Start a shuffled playback of that genre's tracks.
    Shuffle(String),
}

/// Persistent Genres-view state plus the name-sorted rows rebuilt from the
/// library snapshot when it changes.
#[derive(Debug, Default)]
pub struct GenresView {
    /// Genres in display order, rebuilt by [`GenresView::refresh`].
    rows: Vec<GenreInfo>,
    /// The library revision the rows were built from; `None` when the backend
    /// provides no cheap signal or the rows are stale.
    source_revision: Option<u64>,
    /// The search revision the rows were last filtered for.
    source_search: u64,
    /// Bumped whenever what the view displays changes.
    revision: u64,
}

impl GenresView {
    /// Rebuilds the name-sorted rows from the library snapshot in `cx`,
    /// keeping only genres whose name matches the active search or that have a
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

        let mut rows = library.genres().to_vec();
        if let Some(search) = cx.search.filter(|search| search.is_active()) {
            let matching = matching_genres(library, search);
            rows.retain(|genre| {
                filter::name_kept(Some(search), &genre.name)
                    || matching.contains(&genre.name.to_lowercase())
            });
        }
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        if rows != self.rows {
            self.rows = rows;
            self.revision += 1;
        }
    }

    /// The genre rows in display order.
    pub fn rows(&self) -> &[GenreInfo] {
        &self.rows
    }

    /// The number of genres shown.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no genres are shown.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The header count text, e.g. `"15 genres"`.
    pub fn count_label(&self) -> String {
        format!("{} genres", self.rows.len())
    }

    /// The revision counter, bumped whenever the displayed rows change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies one user intent, queueing any resulting commands. `cx` must
    /// carry the library snapshot ([`Ctx::with_library`]).
    pub fn update(&mut self, msg: GenresMsg, cx: &Ctx, out: &mut Commands) {
        match msg {
            GenresMsg::Shuffle(name) => {
                let Some(library) = cx.library else {
                    return;
                };
                let ids: Vec<u64> = library
                    .tracks()
                    .iter()
                    .filter(|track| genre_matches(&track.genre, &name))
                    .map(|track| track.id)
                    .collect();
                if !ids.is_empty() {
                    out.push(Command::ShuffleScope {
                        ids,
                        label: format!("Genre — {name}"),
                    });
                }
            }
        }
    }
}

/// Whether a raw genre tag contains `name` as one of its `;`/`/`/`, `-split
/// parts, matching the library index and the old `shuffle::genre` rule.
fn genre_matches(tag: &str, name: &str) -> bool {
    genre_parts(tag).any(|part| part.eq_ignore_ascii_case(name))
}

/// The trimmed, non-empty parts of a raw genre tag.
fn genre_parts(tag: &str) -> impl Iterator<Item = &str> {
    tag.split(&[';', '/', ','][..])
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

/// The lower-cased genre names that have at least one track matching
/// `search`, so a genre row survives when one of its tracks does.
fn matching_genres(library: &dyn LibraryDataSource, search: &SearchEngine) -> HashSet<String> {
    let mut names = HashSet::new();
    for track in library.tracks() {
        if !search.is_match(track.id) {
            continue;
        }
        for part in genre_parts(&track.genre) {
            names.insert(part.to_lowercase());
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_api::LibraryDataSource;
    use crate::mock::MockLibrary;
    use crate::views::test_library::RiggedLibrary;

    fn view_with_library(library: &dyn LibraryDataSource) -> GenresView {
        let mut view = GenresView::default();
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

    /// A library with genres "Rock"/"Jazz"/"Ambient" and a track in Rock and
    /// one in Jazz.
    fn genre_library() -> RiggedLibrary {
        let mut library = RiggedLibrary::new(1);
        library.genres = vec![
            RiggedLibrary::genre("Rock"),
            RiggedLibrary::genre("Jazz"),
            RiggedLibrary::genre("Ambient"),
        ];
        library.tracks = vec![
            crate::library_api::TrackInfo {
                id: 1,
                title: "One".to_string(),
                genre: "Rock".to_string(),
                ..crate::library_api::TrackInfo::default()
            },
            crate::library_api::TrackInfo {
                id: 2,
                title: "Two".to_string(),
                genre: "Jazz".to_string(),
                ..crate::library_api::TrackInfo::default()
            },
        ];
        library
    }

    #[test]
    fn refresh_sorts_genres_by_name_and_counts_them() {
        let library = MockLibrary::new();
        let view = view_with_library(&library);

        let names: Vec<&str> = view.rows().iter().map(|g| g.name.as_str()).collect();
        let mut expected = names.clone();
        expected.sort();
        assert_eq!(names, expected);
        assert_eq!(view.len(), library.genres().len());
        assert_eq!(
            view.count_label(),
            format!("{} genres", library.genres().len())
        );
    }

    #[test]
    fn refresh_rebuilds_only_on_a_revision_change() {
        let mut library = RiggedLibrary::new(1);
        library.genres = vec![
            RiggedLibrary::genre("Rock"),
            RiggedLibrary::genre("Ambient"),
        ];

        let mut view = GenresView::default();
        {
            let cx = Ctx::with_library(&[], None, &library);
            view.refresh(&cx);
            assert_eq!(view.len(), 2);
            assert_eq!(view.rows()[0].name, "Ambient", "rows are sorted");

            let reads = library.genre_reads.get();
            let revision = view.revision();
            view.refresh(&cx);
            assert_eq!(
                library.genre_reads.get(),
                reads,
                "an unchanged revision re-reads nothing"
            );
            assert_eq!(
                view.revision(),
                revision,
                "an unchanged revision doesn't bump"
            );
        }

        library.genres.push(RiggedLibrary::genre("Jazz"));
        library.revision = Some(2);
        let before = view.revision();
        view.refresh(&Ctx::with_library(&[], None, &library));
        assert_eq!(view.len(), 3, "a changed revision rebuilds");
        assert_eq!(view.rows()[2].name, "Rock");
        assert!(
            view.revision() > before,
            "a changed list bumps the revision"
        );
    }

    #[test]
    fn shuffle_resolves_the_genres_track_ids() {
        let library = MockLibrary::new();
        let mut view = view_with_library(&library);
        let name = view.rows()[0].name.clone();
        let expected: Vec<u64> = library
            .tracks()
            .iter()
            .filter(|track| genre_matches(&track.genre, &name))
            .map(|track| track.id)
            .collect();

        let mut out = Commands::new();
        view.update(
            GenresMsg::Shuffle(name.clone()),
            &Ctx::with_library(&[], None, &library),
            &mut out,
        );
        assert_eq!(
            out.into_vec(),
            vec![Command::ShuffleScope {
                ids: expected,
                label: format!("Genre — {name}"),
            }]
        );
    }

    #[test]
    fn shuffle_without_matching_tracks_is_a_no_op() {
        let library = MockLibrary::new();
        let mut view = view_with_library(&library);
        let mut out = Commands::new();
        view.update(
            GenresMsg::Shuffle("No Such Genre".to_string()),
            &Ctx::with_library(&[], None, &library),
            &mut out,
        );
        assert!(out.is_empty(), "an empty scope is never queued");
    }

    #[test]
    fn genre_matches_any_split_part_case_insensitively() {
        assert!(genre_matches("Jazz; Live", "live"));
        assert!(genre_matches("Rock/Pop", "pop"));
        assert!(genre_matches("Electronic, Ambient", "ambient"));
        assert!(!genre_matches("Jazz", "Rock"));
    }

    #[test]
    fn search_keeps_genres_by_name_or_matching_track_and_restores_on_clear() {
        let library = genre_library();
        let mut engine = SearchEngine::new();
        settle(&mut engine, &library.tracks, "one");

        let mut view = GenresView::default();
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        let names: Vec<&str> = view.rows().iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Rock"], "only the genre of the matching track");

        settle(&mut engine, &library.tracks, "jazz");
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        let names: Vec<&str> = view.rows().iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Jazz"], "the genre name matches too");

        settle(&mut engine, &library.tracks, "");
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        assert_eq!(view.len(), 3, "clearing the query restores every genre");
    }

    #[test]
    fn a_query_matching_no_genre_or_track_yields_no_rows() {
        let library = genre_library();
        let mut engine = SearchEngine::new();
        settle(&mut engine, &library.tracks, "zzzz");

        let mut view = GenresView::default();
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        assert!(view.is_empty());
    }

    #[test]
    fn a_search_change_rebuilds_with_an_unchanged_library() {
        let library = genre_library();
        let mut view = GenresView::default();
        view.refresh(&Ctx::with_library(&[], None, &library));
        assert_eq!(view.len(), 3);
        let revision = view.revision();

        let mut engine = SearchEngine::new();
        settle(&mut engine, &library.tracks, "one");
        view.refresh(&Ctx::with_library(&[], None, &library).with_search(&engine));
        assert!(view.revision() > revision);
        assert_eq!(view.len(), 1);
    }
}
