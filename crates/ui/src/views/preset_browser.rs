//! The projectM preset browser model (#338): every scanned preset (bundled
//! packs, downloaded packs and the user's own folder) as a filterable,
//! pack-then-name ordered list, with the selection and the currently showing
//! preset tracked independently of any toolkit.
//!
//! It is fed the same ordered file list that was added to the projectM
//! playlist, so a row's index into [`PresetBrowser::entries`] is exactly the
//! playlist index to play. Rendering (a virtualized [`ListView`] in the app)
//! stays in the frontend; everything here is plain data and rules, unit-tested
//! without a window.
//!
//! [`ListView`]: win32ui::ListView

use std::path::{Path, PathBuf};

/// One preset in the playlist: its file, the pack folder it came from and the
/// display name (its file stem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetEntry {
    /// The preset's `.milk` file.
    pub path: PathBuf,
    /// The display name: the file stem, e.g. `Dancer`.
    pub name: String,
    /// The pack folder the file came from, e.g. `cream-of-the-crop`.
    pub pack: String,
}

impl PresetEntry {
    /// Builds an entry from a file path and the pack folder it was scanned
    /// from; the display name is derived from the path's file stem.
    pub fn new(path: impl Into<PathBuf>, pack: impl Into<String>) -> Self {
        let path = path.into();
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            path,
            name,
            pack: pack.into(),
        }
    }
}

/// The browser's data, filter and selection.
///
/// `entries` keeps the scan (playlist) order, so an entry's index there is its
/// projectM playlist index; `rows` maps the visible, sorted rows back to it.
#[derive(Debug, Default)]
pub struct PresetBrowser {
    /// Every scanned preset, in playlist order.
    entries: Vec<PresetEntry>,
    /// The visible rows: indices into `entries`, filtered and sorted by pack
    /// then name.
    rows: Vec<usize>,
    /// The case-insensitive name/pack filter; empty shows everything.
    filter: String,
    /// The selected preset's file, if any.
    selected_path: Option<PathBuf>,
    /// The file currently showing, if known.
    current_path: Option<PathBuf>,
    /// Bumped whenever `entries` or `rows` changes, so a frontend can tell
    /// when it must rebuild its own list.
    revision: u64,
}

impl PresetBrowser {
    /// An empty browser.
    pub fn new() -> Self {
        Self::default()
    }

    /// A counter bumped whenever the entries or the visible rows change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The number of visible rows (after filtering).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no preset is visible.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The total number of scanned presets, before filtering.
    pub fn total(&self) -> usize {
        self.entries.len()
    }

    /// The entry at visible `row`, or `None` past the end.
    pub fn entry(&self, row: usize) -> Option<&PresetEntry> {
        self.rows
            .get(row)
            .and_then(|&index| self.entries.get(index))
    }

    /// The projectM playlist index of visible `row` (its index in the scanned
    /// playlist order), or `None` past the end.
    pub fn playlist_index(&self, row: usize) -> Option<usize> {
        self.rows.get(row).copied()
    }

    /// The current filter text.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// The visible row that is selected, if any.
    pub fn selected(&self) -> Option<usize> {
        let path = self.selected_path.as_deref()?;
        self.row_of(path)
    }

    /// The selected preset, if any.
    pub fn selected_entry(&self) -> Option<&PresetEntry> {
        self.selected().and_then(|row| self.entry(row))
    }

    /// Selects visible `row`; out-of-range rows clear the selection. The model
    /// does not bump [`Self::revision`] for a selection, so a frontend driving
    /// its own selection never rebuilds the list.
    pub fn select(&mut self, row: Option<usize>) {
        self.selected_path = row
            .and_then(|row| self.entry(row))
            .map(|entry| entry.path.clone());
    }

    /// Replaces the filter text and recomputes the visible rows, keeping the
    /// selection if the selected preset still matches.
    pub fn set_filter(&mut self, filter: &str) {
        if self.filter == filter {
            return;
        }
        self.filter = filter.to_owned();
        self.rebuild_rows();
        self.revision += 1;
    }

    /// Records which preset is currently showing, resolved from its file path.
    pub fn set_current(&mut self, path: Option<&Path>) {
        self.current_path = path.map(Path::to_path_buf);
    }

    /// The visible row of the currently showing preset, if it is in the list
    /// and passes the filter.
    pub fn current_row(&self) -> Option<usize> {
        let path = self.current_path.as_deref()?;
        self.row_of(path)
    }

    /// The currently showing preset, if it is in the scanned list.
    pub fn current_entry(&self) -> Option<&PresetEntry> {
        let path = self.current_path.as_deref()?;
        self.entries.iter().find(|entry| entry.path == path)
    }

    /// The projectM playlist index of the currently showing preset, if it is
    /// in the scanned list.
    pub fn current_playlist_index(&self) -> Option<usize> {
        let path = self.current_path.as_deref()?;
        self.entries.iter().position(|entry| entry.path == path)
    }

    /// Replaces the scanned entries (in playlist order), recomputing the
    /// visible rows. The selection and the current preset are re-resolved by
    /// path, so both survive a rescan while their files still exist.
    pub fn rebuild(&mut self, entries: &[PresetEntry]) {
        self.entries = entries.to_vec();
        self.rebuild_rows();
        self.revision += 1;
    }

    /// Recomputes `rows` from `entries` and `filter`, sorted by pack then name
    /// (case-insensitively, with the path as a stable tie-break).
    fn rebuild_rows(&mut self) {
        let needle = self.filter.to_lowercase();
        let mut rows: Vec<usize> = (0..self.entries.len())
            .filter(|&index| matches_filter(&self.entries[index], &needle))
            .collect();
        rows.sort_by(|&a, &b| compare_entries(&self.entries[a], &self.entries[b]));
        self.rows = rows;
    }

    /// The visible row showing `path`, if it is present.
    fn row_of(&self, path: &Path) -> Option<usize> {
        self.rows
            .iter()
            .position(|&index| self.entries[index].path == path)
    }
}

/// Whether `entry` matches the lowercased `needle` by name or pack; an empty
/// needle matches everything.
fn matches_filter(entry: &PresetEntry, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    entry.name.to_lowercase().contains(needle) || entry.pack.to_lowercase().contains(needle)
}

/// Orders presets by pack, then name (both case-insensitively), with the path
/// as a stable tie-break so two same-named presets in one pack stay put.
fn compare_entries(a: &PresetEntry, b: &PresetEntry) -> std::cmp::Ordering {
    a.pack
        .to_lowercase()
        .cmp(&b.pack.to_lowercase())
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then_with(|| a.path.cmp(&b.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(pack: &str, name: &str) -> PresetEntry {
        PresetEntry::new(PathBuf::from(pack).join(format!("{name}.milk")), pack)
    }

    fn sample() -> Vec<PresetEntry> {
        // Deliberately unsorted; `rebuild` must order by pack then name.
        vec![
            entry("milkdrop-original", "Zebra"),
            entry("cream-of-the-crop", "Dancer"),
            entry("milkdrop-original", "Alpha"),
            entry("en-d", "Pulse"),
        ]
    }

    fn names(browser: &PresetBrowser) -> Vec<(&str, &str)> {
        (0..browser.len())
            .filter_map(|row| browser.entry(row))
            .map(|entry| (entry.pack.as_str(), entry.name.as_str()))
            .collect()
    }

    #[test]
    fn empty_browser_has_nothing() {
        let browser = PresetBrowser::new();
        assert!(browser.is_empty());
        assert_eq!(browser.total(), 0);
        assert_eq!(browser.selected(), None);
        assert_eq!(browser.current_row(), None);
    }

    #[test]
    fn entries_are_ordered_by_pack_then_name() {
        let mut browser = PresetBrowser::new();
        browser.rebuild(&sample());
        assert_eq!(
            names(&browser),
            vec![
                ("cream-of-the-crop", "Dancer"),
                ("en-d", "Pulse"),
                ("milkdrop-original", "Alpha"),
                ("milkdrop-original", "Zebra"),
            ]
        );
    }

    #[test]
    fn playlist_index_follows_the_scanned_order() {
        let entries = sample();
        let mut browser = PresetBrowser::new();
        browser.rebuild(&entries);
        // The first visible row is "Dancer", which was the second scanned file.
        assert_eq!(browser.playlist_index(0), Some(1));
        assert_eq!(browser.entry(0).map(|e| e.name.as_str()), Some("Dancer"));
    }

    #[test]
    fn filter_matches_name_and_pack_case_insensitively() {
        let mut browser = PresetBrowser::new();
        browser.rebuild(&sample());

        browser.set_filter("DANCER");
        assert_eq!(names(&browser), vec![("cream-of-the-crop", "Dancer")]);

        browser.set_filter("milkdrop");
        assert_eq!(browser.len(), 2);
        assert!(
            (0..browser.len())
                .filter_map(|row| browser.entry(row))
                .all(|entry| entry.pack == "milkdrop-original")
        );

        browser.set_filter("alpha");
        assert_eq!(names(&browser), vec![("milkdrop-original", "Alpha")]);

        browser.set_filter("");
        assert_eq!(browser.len(), 4, "an empty filter shows everything");
        assert_eq!(browser.filter(), "");
    }

    #[test]
    fn selection_survives_a_rescan_by_path() {
        let entries = sample();
        let mut browser = PresetBrowser::new();
        browser.rebuild(&entries);
        browser.set_filter("alpha");
        browser.select(Some(0));
        assert_eq!(
            browser.selected_entry().map(|entry| entry.name.as_str()),
            Some("Alpha")
        );

        // A rescan with the same files keeps the selection by path, though a
        // new preset shifts the row indices.
        let mut rescanned = entries.clone();
        rescanned.push(entry("cream-of-the-crop", "Aardvark"));
        rescanned.sort_by(compare_entries);
        browser.rebuild(&rescanned);

        let selected = browser.selected().expect("the selection survived");
        assert_eq!(
            browser.entry(selected).map(|entry| entry.name.as_str()),
            Some("Alpha"),
            "the selection follows the file, not its old index"
        );
    }

    #[test]
    fn filtering_out_the_selection_clears_it() {
        let mut browser = PresetBrowser::new();
        browser.rebuild(&sample());
        browser.select(Some(0));
        assert!(browser.selected().is_some());

        browser.set_filter("nothing-matches");
        assert!(browser.is_empty());
        assert_eq!(browser.selected(), None, "the selected row is gone");
    }

    #[test]
    fn current_resolves_from_the_last_preset_path() {
        let entries = sample();
        let mut browser = PresetBrowser::new();
        browser.rebuild(&entries);

        browser.set_current(Some(Path::new("milkdrop-original/Alpha.milk")));
        let row = browser
            .current_row()
            .expect("the current preset is visible");
        assert_eq!(browser.entry(row).map(|e| e.name.as_str()), Some("Alpha"));

        // A filter that hides it makes `current_row` `None`, though the current
        // preset itself is still resolved.
        browser.set_filter("dancer");
        assert_eq!(browser.current_row(), None);
        assert_eq!(
            browser.current_entry().map(|e| e.name.as_str()),
            Some("Alpha")
        );

        // An unknown path resolves to nothing.
        browser.set_current(Some(Path::new("nope/missing.milk")));
        assert_eq!(browser.current_row(), None);
        assert!(browser.current_entry().is_none());
    }

    #[test]
    fn revision_tracks_structural_changes_only() {
        let mut browser = PresetBrowser::new();
        let empty = browser.revision();
        browser.rebuild(&sample());
        let rebuilt = browser.revision();
        assert!(rebuilt > empty);

        browser.set_filter("dancer");
        let filtered = browser.revision();
        assert!(filtered > rebuilt, "a filter change rebuilds the rows");
        browser.set_filter("dancer");
        assert_eq!(
            browser.revision(),
            filtered,
            "an unchanged filter is a no-op"
        );

        browser.select(Some(0));
        browser.set_current(Some(Path::new("cream-of-the-crop/Dancer.milk")));
        assert_eq!(
            browser.revision(),
            filtered,
            "selection and current must not force a list rebuild"
        );
    }

    #[test]
    fn selection_out_of_range_clears() {
        let mut browser = PresetBrowser::new();
        browser.rebuild(&sample());
        browser.select(Some(2));
        assert_eq!(browser.selected(), Some(2));
        browser.select(Some(99));
        assert_eq!(browser.selected(), None);
    }
}
