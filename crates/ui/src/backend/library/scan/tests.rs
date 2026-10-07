//! Unit tests for the scan's snapshot policy and status formatting.

use std::time::Duration;

use emusic_library::scanner::ScanSummary;

use super::{Refresh, format_count};

fn summary() -> ScanSummary {
    ScanSummary {
        files_found: 6_209,
        tracks_added: 0,
        tracks_updated: 0,
        tracks_moved: 0,
        tracks_deleted: 0,
        files_skipped: 0,
        modules_skipped: 0,
        unreachable_roots: Vec::new(),
        cancelled: false,
        partial: false,
        elapsed: Duration::ZERO,
    }
}

#[test]
fn unchanged_rescan_needs_no_snapshot() {
    assert!(!Refresh::IfChanged.needed(0, Some(&summary())));
    assert!(!Refresh::IfChanged.needed(0, None));
    // Skipped files and unreachable roots leave the store untouched.
    let partial = ScanSummary {
        files_skipped: 3,
        unreachable_roots: vec!["Z:\\gone".into()],
        partial: true,
        ..summary()
    };
    assert!(!Refresh::IfChanged.needed(0, Some(&partial)));
}

#[test]
fn any_store_change_needs_a_snapshot() {
    let changes = [
        ScanSummary {
            tracks_added: 1,
            ..summary()
        },
        ScanSummary {
            tracks_updated: 1,
            ..summary()
        },
        ScanSummary {
            tracks_moved: 1,
            ..summary()
        },
        ScanSummary {
            tracks_deleted: 1,
            ..summary()
        },
        ScanSummary {
            cancelled: true,
            ..summary()
        },
    ];
    for changed in &changes {
        assert!(Refresh::IfChanged.needed(0, Some(changed)), "{changed:?}");
    }
    assert!(Refresh::IfChanged.needed(2, Some(&summary())));
    assert!(Refresh::IfChanged.needed(2, None));
}

#[test]
fn folder_changes_always_need_a_snapshot() {
    assert!(Refresh::Always.needed(0, Some(&summary())));
    assert!(Refresh::Always.needed(0, None));
}

#[test]
fn format_count_groups_thousands() {
    assert_eq!(format_count(0), "0");
    assert_eq!(format_count(999), "999");
    assert_eq!(format_count(1_000), "1,000");
    assert_eq!(format_count(12_345), "12,345");
    assert_eq!(format_count(1_234_567), "1,234,567");
}
