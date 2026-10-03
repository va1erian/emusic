//! Two-way sync of starred tracks with an `emusic-server` (#516).
//!
//! Runs inside the remote sync worker, right after a server's library delta,
//! on the worker's private store connection:
//!
//! 1. fetch the server's starred set (conditional on the stored version);
//! 2. map desktop tracks to server ids — rows synced from the server by their
//!    id, local files by metadata ([`matcher`]);
//! 3. three-way merge against the last synced set ([`merge`]);
//! 4. push the desktop's changes with `star_batch`, then apply the server's
//!    changes locally and store the new base in one transaction.
//!
//! Starring in the UI only writes the local flag (stamped with the time), so
//! it stays instant and works offline; [`debounce`] schedules a sync shortly
//! after a change.

pub(crate) mod debounce;
pub(crate) mod matcher;
pub(crate) mod merge;

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use emusic_client::RemoteClient;
use emusic_library::{StarSyncRow, StarredBase, Store, TrackId};
use tracing::{debug, info};

use matcher::{MatchFields, Matcher};
use merge::{LocalStar, MergeInput};

/// Most ids the server accepts in one `star_batch` request.
const MAX_BATCH_IDS: usize = 5_000;

/// What one starred sync did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct StarSyncOutcome {
    /// Ids starred or unstarred on the server.
    pub pushed: usize,
    /// Desktop tracks whose star changed.
    pub applied: usize,
}

/// The desktop tracks mapped to one server id.
struct Group {
    star: LocalStar,
    tracks: Vec<(TrackId, bool)>,
}

/// Syncs the starred tracks of `server_id` with the server behind `client`.
pub(crate) fn sync_stars(
    store: &mut Store,
    client: &RemoteClient,
    token: &str,
    server_id: &str,
) -> Result<StarSyncOutcome> {
    let base = store.starred_base(server_id)?;
    let fetched = client
        .starred(token, base.version)
        .context("fetching starred tracks")?;
    // Unchanged (304): the server set is the base. Its star times are only
    // consulted for ids the server changed, so they are not needed.
    let (version, server): (Option<i64>, HashMap<String, i64>) = match fetched {
        Some(set) => (
            Some(set.version),
            set.tracks
                .into_iter()
                .map(|track| (track.id, track.starred_at))
                .collect(),
        ),
        None => (
            base.version,
            base.ids.iter().map(|id| (id.clone(), 0)).collect(),
        ),
    };

    // Read local state after the fetch and remember when: a star changed
    // later is left for the next sync.
    let read_at_ms = unix_now_ms();
    let groups = group_rows(store.star_sync_rows(server_id)?);
    let local: HashMap<String, LocalStar> = groups
        .iter()
        .map(|(id, group)| (id.clone(), group.star))
        .collect();
    let plan = merge::merge(MergeInput {
        base: &base.ids,
        synced_at_ms: base.synced_at_ms,
        server: &server,
        local: &local,
    });

    let unknown = push(client, token, &plan.star, &plan.unstar)?;
    let pushed = plan.star.len().saturating_sub(unknown.len()) + plan.unstar.len();
    let mut starred = plan.starred;
    starred.retain(|id| !unknown.contains(id));

    let changes: Vec<(TrackId, bool)> = plan
        .local
        .iter()
        .filter_map(|(id, merged)| groups.get(id).map(|group| (group, *merged)))
        .flat_map(|(group, merged)| {
            group
                .tracks
                .iter()
                .filter(move |(_, starred)| *starred != merged)
                .map(move |(track, _)| (*track, merged))
        })
        .collect();
    // After a push, keep the pre-push version: the next fetch then returns
    // the full set, so changes other devices made meanwhile are not hidden
    // behind an ETag that already covers them.
    let new_base = StarredBase {
        version,
        synced_at_ms: read_at_ms,
        ids: starred,
    };
    let applied = store.commit_star_sync(server_id, &new_base, &changes)?;
    info!(server_id, pushed, applied, "starred sync finished");
    Ok(StarSyncOutcome { pushed, applied })
}

/// Groups the store rows by the server id they map to. Rows synced from the
/// server map by id; local files by metadata, when the match is unique.
fn group_rows(rows: Vec<StarSyncRow>) -> HashMap<String, Group> {
    let matcher = Matcher::new(rows.iter().filter_map(|row| {
        row.remote_track_id
            .as_deref()
            .map(|id| (id, match_fields(row)))
    }));
    let mut groups: HashMap<String, Group> = HashMap::new();
    for row in &rows {
        let id = match row.remote_track_id.as_deref() {
            Some(id) => id,
            None => match matcher.find(&match_fields(row)) {
                Some(id) => id,
                None => {
                    if row.starred {
                        debug!(
                            track = row.id.0,
                            "starred local track has no unique server match"
                        );
                    }
                    continue;
                }
            },
        };
        groups
            .entry(id.to_string())
            .and_modify(|group| group.star.add(row.starred, row.changed_at_ms))
            .or_insert_with(|| Group {
                star: LocalStar::new(row.starred, row.changed_at_ms),
                tracks: Vec::new(),
            })
            .tracks
            .push((row.id, row.starred));
    }
    groups
}

fn match_fields(row: &StarSyncRow) -> MatchFields<'_> {
    MatchFields {
        title: row.title.as_deref(),
        artist: row.artist.as_deref(),
        album_artist: row.album_artist.as_deref(),
        album: row.album.as_deref(),
        track_no: row.track_no,
    }
}

/// Pushes the desktop's changes in batches the server accepts, returning the
/// ids it did not know (deleted on the server since the last delta).
fn push(
    client: &RemoteClient,
    token: &str,
    star: &[String],
    unstar: &[String],
) -> Result<HashSet<String>> {
    let mut unknown = HashSet::new();
    for chunk in star.chunks(MAX_BATCH_IDS) {
        let result = client
            .star_batch(token, chunk, &[])
            .context("pushing starred tracks")?;
        unknown.extend(result.unknown);
    }
    for chunk in unstar.chunks(MAX_BATCH_IDS) {
        client
            .star_batch(token, &[], chunk)
            .context("pushing unstarred tracks")?;
    }
    Ok(unknown)
}

/// The current time as Unix milliseconds, or 0 when the clock is before 1970.
fn unix_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
