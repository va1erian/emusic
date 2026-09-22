use std::path::PathBuf;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use emusic_core::{ArtSource, Track, TrackId, TrackKind};
use emusic_search::matcher::{Index, PreparedQuery};
use emusic_search::query::parse;

const TRACK_COUNT: usize = 100_000;

fn synthetic_track(index: usize) -> Track {
    let id = TrackId((index + 1) as i64);
    let artist = if index.is_multiple_of(2) {
        Some("John Smith".to_string())
    } else {
        Some("Jane Doe".to_string())
    };
    let title = if index.is_multiple_of(7) {
        Some("Live Performance".to_string())
    } else {
        Some(format!("Studio Track {}", index))
    };
    let album = Some(format!("Album {}", index % 200));
    let genre = Some(
        match index % 5 {
            0 => "Rock",
            1 => "Pop",
            2 => "Electronic",
            3 => "Jazz",
            _ => "Classical",
        }
        .to_string(),
    );
    let year = Some(1980 + (index % 50) as i32);

    Track {
        id,
        path: PathBuf::from(format!(r"C:\music\{}\{:08}.flac", index % 100, index)),
        dir: PathBuf::from(format!(r"C:\music\{}", index % 100)),
        filename: format!("{:08}.flac", index),
        ext: "flac".to_string(),
        size: 10_000_000 + (index as u64),
        mtime: 1_700_000_000,
        kind: TrackKind::Stream,
        duration_ms: 180_000 + ((index % 300) as u32) * 1000,
        bitrate: Some(1_000),
        sample_rate: Some(44_100),
        channels: Some(2),
        title,
        artist,
        album_artist: None,
        album,
        genre,
        year,
        track_no: Some((index % 20 + 1) as u32),
        disc_no: None,
        composer: if index.is_multiple_of(11) {
            Some("Famous Composer".to_string())
        } else {
            None
        },
        comment: None,
        art_source: ArtSource::None,
        added_at: 1_700_000_000,
    }
}

fn build_index() -> Index {
    let mut index = Index::new();
    for i in 0..TRACK_COUNT {
        index.add_track(synthetic_track(i));
    }
    index
}

fn bench_three_term_query(c: &mut Criterion) {
    let index = build_index();
    let query = parse("artist:smith year:1990..2010 live");
    let prepared = PreparedQuery::new(&query);
    let candidates: Vec<TrackId> = (1..=TRACK_COUNT as i64).map(TrackId::from).collect();
    let stats = |_id: TrackId| 0u64;

    c.bench_function("search 100k 3-term", |b| {
        b.iter(|| {
            index.search_prepared(
                black_box(&prepared),
                black_box(&stats),
                black_box(&candidates),
            )
        })
    });
}

criterion_group!(benches, bench_three_term_query);
criterion_main!(benches);
