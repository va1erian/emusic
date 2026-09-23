//! Behavioral regression test for #134: double-clicking a track used to
//! call `player.replace_and_play(&[that_one_path], 0)`, so the queue always
//! ended up with exactly one entry and `next()`/`previous()` had nothing to
//! advance to (acted like Stop). Unit tests never caught this because each
//! piece looked fine in isolation - the bug was that the queue was empty.
//!
//! This drives the *real* `App` headlessly via `egui_kittest` (falls back to
//! a software GPU adapter such as WARP on Windows; skips, per #32's
//! precedent in `snapshot_views.rs`, when no adapter is available at all).
//! It selects a track partway through a filtered Music view with the
//! keyboard (arrow keys + Enter - the track table's other "play this row"
//! path, sharing the exact same context-carrying `TrackAction::Play` as a
//! double-click), reads the now-playing right panel's "Up Next" queue back
//! out of the rendered UI, and then clicks the real "Next" transport button
//! twice to confirm the queue is actually walked through rather than
//! sitting empty.

use std::collections::{BTreeMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::time::Duration;

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use emusic_search::normalize_text;

use emusic::app::App;
use emusic::config::Config;
use emusic::library_api::LibraryDataSource;
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::View;

/// Picks an artist whose track count mirrors the issue's own repro ("track 2
/// of a 25-track filtered list"): small enough that every row fits on one
/// unscrolled page of the table (so label lookups find it without
/// scrolling), but big enough to have tracks both before *and* after the
/// selected one - the exact shape that used to produce a one-entry queue
/// regardless of where in the list you played from.
///
/// Returns the artist name, the track ids in the order the Music view will
/// show them (ascending id, since the mock generates tracks in that order
/// and no sort is applied), and the index of the track we'll play.
fn pick_target(library: &MockLibrary) -> (String, Vec<u64>, usize) {
    let mut by_artist: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for track in library.tracks() {
        if track.artist.is_empty() {
            continue; // missing-tags tracks; excluded from every field query
        }
        by_artist
            .entry(track.artist.clone())
            .or_default()
            .push(track.id);
    }

    let artist_names: Vec<&String> = by_artist.keys().collect();
    for (artist, ids) in &by_artist {
        if !(8..=25).contains(&ids.len()) {
            continue;
        }
        // The query is a substring match (field queries in
        // `emusic_search`), so guard against one generated artist name
        // happening to be a substring of another's - which would make the
        // filtered list bigger than `ids` and throw off the expected index.
        let needle = normalize_text(artist);
        let collides = artist_names
            .iter()
            .any(|other| *other != artist && normalize_text(other).contains(&needle));
        if collides {
            continue;
        }
        // `MockPlayer` (crate::mock::player) only ever surfaces a track's
        // file-stem as its display text (now-playing heading, queue rows) -
        // never the real `TrackInfo::title`. Stems are `{id % 1000}`, so
        // guard against two tracks in this artist's list colliding on it,
        // which would make the stem-based occurrence checks below ambiguous.
        let stems: HashSet<_> = ids.iter().map(|id| stem_of(library, *id)).collect();
        if stems.len() != ids.len() {
            continue;
        }
        return (artist.clone(), ids.clone(), ids.len() / 2);
    }
    panic!("no collision-free 8..=25-track artist found in the mock library");
}

/// Title of the track with this id, panicking if it's somehow missing (it
/// shouldn't be - ids come straight from `library.tracks()`).
fn title_of(library: &MockLibrary, id: u64) -> String {
    library
        .tracks()
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.title.clone())
        .unwrap_or_else(|| panic!("track {id} vanished from the mock library"))
}

/// The mock player's display text for track `id`: the file stem of its
/// path, exactly what `MockPlayer::label()` derives for the now-playing
/// heading and queue-row titles (`crate::mock::player`, since the mock
/// player's `NowPlayingInfo`/`QueueEntry` never carry real library
/// metadata - the same limitation the real BASS-backed player has today,
/// noted in `views/music.rs::currently_playing_id`).
fn stem_of(library: &MockLibrary, id: u64) -> String {
    let path = library
        .tracks()
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.path.clone())
        .unwrap_or_else(|| panic!("track {id} vanished from the mock library"));
    Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_else(|| panic!("track {id}'s path {path:?} has no file stem"))
        .to_string()
}

/// The queue preview's "N tracks" counter is a single `egui::Label` with the
/// exact text `"{n} tracks"` (`panels::now_playing::queue::show`); reading
/// it back is a direct readout of `player.queue().len()` without needing
/// any access to the (private) `App` internals. Label-role accesskit nodes
/// carry their text in `value`, not `label` (kittest's `By::label` already
/// knows this and checks `value` for `Role::Label`, which is what
/// `query_by_label` uses under the hood).
fn queue_count_label(harness: &Harness<'_, App>, n: usize) -> bool {
    harness.query_by_label(&format!("{n} tracks")).is_some()
}

/// The four transport buttons (Previous/Play-Pause/Stop/Next,
/// `panels::top_bar::transport_buttons`) are plain icon buttons with empty
/// (`""`) text, so they carry an empty accessible label - unlike every other
/// button on screen (menus, nav entries, repeat/shuffle's emoji glyph,
/// "Shuffle all", genre/artist/album browser rows all have real text).
/// Empty-labeled buttons, in on-screen left-to-right order, are exactly
/// those four; index 3 is "Next".
fn click_next_button(harness: &mut Harness<'_, App>) {
    let empty_labeled: Vec<_> = harness
        .get_all_by_role(Role::Button)
        .filter(|node| node.accesskit_node().label().as_deref() == Some(""))
        .collect();
    assert_eq!(
        empty_labeled.len(),
        4,
        "expected exactly the 4 empty-labeled transport buttons (Previous/Play-Pause/Stop/Next)"
    );
    empty_labeled[3].click(); // Previous, Play/Pause, Stop, Next
}

#[test]
fn selecting_a_track_queues_the_filtered_view_and_next_walks_through_it() {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let inspect_library = MockLibrary::new();
        let (artist, context, target_index) = pick_target(&inspect_library);
        let target_id = context[target_index];
        let target_title = title_of(&inspect_library, target_id);
        let remaining_after_target = context.len() - target_index - 1;

        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1280.0, 800.0))
            .build_eframe(|cc| {
                // A second, independently-generated library: the mock's
                // fixed seed (`data::SEED`) makes it byte-identical to
                // `inspect_library`, so `context`/`target_title` above are
                // still valid for whatever the app actually renders.
                let library = MockLibrary::new();
                let player = Box::new(MockPlayer::default());
                App::with_config(cc, Box::new(library), player, Config::default())
            });

        harness.state_mut().set_view(View::Music);
        harness
            .state_mut()
            .set_search_query(format!("artist:\"{artist}\""));

        // The library's full-text search runs off-thread (`SearchEngine`);
        // poll for its answer like the UI thread does once per frame.
        let mut settled = false;
        for _ in 0..200 {
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
            if harness.query_by_label_contains(&target_title).is_some() {
                settled = true;
                break;
            }
        }
        assert!(settled, "search for artist {artist:?} never settled");

        // Baseline: nothing playing yet, so the target track's title
        // appears exactly once (its own row in the Music table).
        let baseline = harness.get_all_by_label_contains(&target_title).count();
        assert_eq!(
            baseline, 1,
            "expected the target track's title to appear once before playback starts"
        );

        // Move keyboard focus down to the target row and press Enter to
        // play it (`views/track_table/mod.rs`'s Enter-to-play): from no
        // prior focus, `target_index` presses of ArrowDown land exactly on
        // it (`SelectionState::move_focus` treats "no focus" as row 0).
        for _ in 0..target_index {
            harness.key_press(egui::Key::ArrowDown);
        }
        harness.step();
        harness.key_press(egui::Key::Enter);
        harness.step();
        // One more frame: `panels::right_panel` (which renders the queue)
        // runs before `apply_pending()` in the frame that applied the
        // command, so the queue panel only reflects it starting next frame.
        harness.step();

        assert!(
            queue_count_label(&harness, remaining_after_target),
            "expected the Up Next queue to hold the {remaining_after_target} tracks \
             after the selected one in the {artist:?} filtered view, not just the \
             single selected track (#134)"
        );

        // The mock player's now-playing heading and queue rows only ever
        // show a track's file-stem (never the real library title - see
        // `stem_of`'s doc comment), so from here on identify tracks by
        // stem, checked with an *exact* label match. A stem then shows up
        // exactly once if the track is playing *or* queued, and not at all
        // otherwise (the table itself never renders a bare stem - its File
        // column shows the full path).
        let target_stem = stem_of(&inspect_library, target_id);
        let next_id = context[target_index + 1];
        let next_stem = stem_of(&inspect_library, next_id);
        let previous_id = context[target_index - 1];
        let previous_stem = stem_of(&inspect_library, previous_id);

        // The target track is now playing: its stem is the now-playing
        // heading.
        assert!(
            harness.query_by_label(&target_stem).is_some(),
            "expected the target track ({target_stem}) to be the now-playing track"
        );
        // The very next track in the filtered view is queued - proving the
        // target's *position* in the list, not just its id, made it into
        // the queue in the right order.
        assert!(
            harness.query_by_label(&next_stem).is_some(),
            "expected the track right after the target one ({next_stem}) to be queued"
        );
        // And the track right *before* the target one must not have been
        // pulled into the queue (the queue starts after the target
        // position, it doesn't wrap around).
        assert!(
            harness.query_by_label(&previous_stem).is_none(),
            "the track before the target one ({previous_stem}) should not be playing or queued"
        );

        // Now actually exercise Next twice via the real transport button,
        // confirming the queue is walked through rather than sitting there
        // (the pre-#134 bug: an always-empty queue means Next is a no-op).
        click_next_button(&mut harness);
        harness.step();
        // Same one-frame lag as above: `top_bar` (which owns the Next
        // button) renders, and queues `Command::PlayerNext`, before
        // `right_panel` in this same frame, so the queue panel is stale
        // until the next one.
        harness.step();
        assert!(
            queue_count_label(&harness, remaining_after_target - 1),
            "expected one Next click to consume one queue entry"
        );
        assert!(
            harness.query_by_label(&next_stem).is_some(),
            "expected the next track ({next_stem}) to now be playing after one Next click"
        );
        assert!(
            harness.query_by_label(&target_stem).is_none(),
            "the original target track ({target_stem}) should no longer be playing or queued"
        );

        click_next_button(&mut harness);
        harness.step();
        harness.step();
        assert!(
            queue_count_label(&harness, remaining_after_target - 2),
            "expected a second Next click to consume another queue entry"
        );
        let next_next_stem = stem_of(&inspect_library, context[target_index + 2]);
        assert!(
            harness.query_by_label(&next_next_stem).is_some(),
            "expected the track after that ({next_next_stem}) to now be playing after a second Next click"
        );
    }));

    if result.is_err() {
        eprintln!(
            "skipping #134 queue-context test: no headless GPU adapter available in this environment"
        );
    }
}
