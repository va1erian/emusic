//! Genres-view model (#104): the name-sorted genre list with its track
//! counts, plus the per-row "Shuffle play" intent. Rendering (the table and
//! its context menu) stays in the frontends.
//!
//! [`GenresView::refresh`] rebuilds the rows only when the library's
//! [`LibraryDataSource::revision`] counter changes, so an unchanged library
//! does no allocation or sorting per frame. User intents arrive as
//! [`GenresMsg`].

use crate::library_api::GenreInfo;
use crate::state::Command;
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
    /// Bumped whenever what the view displays changes.
    revision: u64,
}

impl GenresView {
    /// Rebuilds the name-sorted rows from the library snapshot in `cx` when
    /// the library's revision changed since the last call.
    pub fn refresh(&mut self, cx: &Ctx) {
        let Some(library) = cx.library else {
            return;
        };
        if let Some(revision) = library.revision() {
            if self.source_revision == Some(revision) {
                return;
            }
            self.source_revision = Some(revision);
        } else {
            self.source_revision = None;
        }

        let mut rows = library.genres().to_vec();
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
    tag.split(&[';', '/', ','][..])
        .any(|part| part.trim().eq_ignore_ascii_case(name))
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
}
