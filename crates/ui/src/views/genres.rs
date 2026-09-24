//! Genres-view model (#104): the name-sorted genre list with its track
//! counts, plus the per-row "Shuffle play" intent. Rendering (the table and
//! its context menu) stays in the frontends.
//!
//! The rows are rebuilt from the library snapshot each frame by
//! [`GenresView::refresh`]; user intents arrive as [`GenresMsg`].

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
/// library snapshot each frame.
#[derive(Debug, Default)]
pub struct GenresView {
    /// Genres in display order, rebuilt by [`GenresView::refresh`].
    rows: Vec<GenreInfo>,
    /// Bumped whenever what the view displays changes.
    revision: u64,
}

impl GenresView {
    /// Rebuilds the name-sorted rows from the library snapshot in `cx`.
    pub fn refresh(&mut self, cx: &Ctx) {
        let Some(library) = cx.library else {
            return;
        };
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

    fn view_with_library(library: &dyn crate::library_api::LibraryDataSource) -> GenresView {
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
    fn refresh_bumps_revision_only_on_change() {
        let library = MockLibrary::new();
        let mut view = GenresView::default();
        let cx = Ctx::with_library(&[], None, &library);
        let before = view.revision();
        view.refresh(&cx);
        assert!(view.revision() > before);
        let after = view.revision();
        view.refresh(&cx);
        assert_eq!(view.revision(), after, "stable rows don't bump");
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
    fn genre_matches_any_split_part_case_insensitively() {
        assert!(genre_matches("Jazz; Live", "live"));
        assert!(genre_matches("Rock/Pop", "pop"));
        assert!(genre_matches("Electronic, Ambient", "ambient"));
        assert!(!genre_matches("Jazz", "Rock"));
    }
}
