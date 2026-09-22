use super::*;

#[test]
fn year_equal_matches() {
    let spec = NumericSpec::Compare {
        op: Comparison::Equal,
        value: 1994.0,
    };
    assert!(matches(spec, 1994.0));
    assert!(!matches(spec, 1995.0));
}

#[test]
fn duration_less_than_matches() {
    let spec = NumericSpec::Compare {
        op: Comparison::Less,
        value: 180.0,
    };
    assert!(matches(spec, 90.0));
    assert!(!matches(spec, 180.0));
    assert!(!matches(spec, 300.0));
}

#[test]
fn closed_range_matches() {
    let spec = NumericSpec::Range {
        min: Some(1990.0),
        max: Some(1999.0),
    };
    assert!(!matches(spec, 1989.0));
    assert!(matches(spec, 1990.0));
    assert!(matches(spec, 1995.0));
    assert!(matches(spec, 1999.0));
    assert!(!matches(spec, 2000.0));
}

#[test]
fn open_range_min_only() {
    let spec = NumericSpec::Range {
        min: Some(10.0),
        max: None,
    };
    assert!(!matches(spec, 9.0));
    assert!(matches(spec, 10.0));
    assert!(matches(spec, 100.0));
}

#[test]
fn open_range_max_only() {
    let spec = NumericSpec::Range {
        min: None,
        max: Some(10.0),
    };
    assert!(matches(spec, 0.0));
    assert!(matches(spec, 10.0));
    assert!(!matches(spec, 11.0));
}

#[test]
fn missing_year_is_nan() {
    let track = emusic_core::Track {
        year: None,
        ..test_track()
    };
    assert_eq!(track_value(Field::Year, &track), None);
}

#[test]
fn duration_converts_from_ms() {
    let mut track = test_track();
    track.duration_ms = 90_500;
    assert_eq!(track_value(Field::Duration, &track), Some(90.5));
}

fn test_track() -> emusic_core::Track {
    use std::path::PathBuf;
    emusic_core::Track {
        id: emusic_core::TrackId::UNASSIGNED,
        path: PathBuf::from(r"C:\music\track.flac"),
        dir: PathBuf::from(r"C:\music"),
        filename: "track.flac".to_string(),
        ext: "flac".to_string(),
        size: 0,
        mtime: 0,
        kind: emusic_core::TrackKind::Stream,
        duration_ms: 0,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: None,
        artist: None,
        album_artist: None,
        album: None,
        genre: None,
        year: None,
        track_no: None,
        disc_no: None,
        composer: None,
        comment: None,
        art_source: emusic_core::ArtSource::None,
        added_at: 0,
    }
}
