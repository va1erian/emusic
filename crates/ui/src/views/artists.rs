//! Artists-view model (#104): the name-sorted artist list with its album and
//! track counts, plus the per-row "Shuffle play" intent. Rendering (the table
//! and its context menu) stays in the frontends.
//!
//! [`ArtistsView::refresh`] rebuilds the rows only when the library's
//! [`LibraryDataSource::revision`] counter changes, so an unchanged library
//! does no allocation or sorting per frame. User intents arrive as
//! [`ArtistsMsg`].

use crate::library_api::ArtistInfo;
use crate::state::Command;
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
    /// Bumped whenever what the view displays changes.
    revision: u64,
}

impl ArtistsView {
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

        let mut rows = library.artists().to_vec();
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
}
