//! The three-way merge of a server's starred set with the desktop's stars
//! (#516).
//!
//! For every server track id the merge looks at three states: the base (the
//! server set as of the last successful sync), the server's current set and
//! the desktop's stars on the tracks mapped to that id. A side that differs
//! from the base changed it; a local star toggled since the last sync also
//! counts as a local change even when it ended where it started. When only
//! one side changed, it wins; when both did, the most recent action wins.

use std::collections::{HashMap, HashSet};

/// The desktop's star on the tracks mapped to one server id.
///
/// Several tracks can map to one id (duplicates of a song); the most recently
/// changed one speaks for the group, so the merge result can then be applied
/// to all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LocalStar {
    /// Whether the group counts as starred.
    pub starred: bool,
    /// The latest local change (Unix ms), if any is known.
    pub changed_at_ms: Option<i64>,
}

impl LocalStar {
    /// The state of a single track.
    pub(crate) fn new(starred: bool, changed_at_ms: Option<i64>) -> Self {
        Self {
            starred,
            changed_at_ms,
        }
    }

    /// Folds another track of the group in: a more recent change takes over;
    /// on a tie (including no stamps at all) a star wins.
    pub(crate) fn add(&mut self, starred: bool, changed_at_ms: Option<i64>) {
        match changed_at_ms.cmp(&self.changed_at_ms) {
            std::cmp::Ordering::Greater => *self = Self::new(starred, changed_at_ms),
            std::cmp::Ordering::Equal => self.starred |= starred,
            std::cmp::Ordering::Less => {}
        }
    }
}

/// The merge inputs for one server.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MergeInput<'a> {
    /// The starred ids as of the last successful sync.
    pub base: &'a HashSet<String>,
    /// When local state was read for the last sync (Unix ms).
    pub synced_at_ms: i64,
    /// The server's current starred ids, with when each was starred (Unix s).
    pub server: &'a HashMap<String, i64>,
    /// The desktop's star per mapped server id.
    pub local: &'a HashMap<String, LocalStar>,
}

/// What a merge decided.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MergePlan {
    /// Ids to star on the server (sorted).
    pub star: Vec<String>,
    /// Ids to unstar on the server (sorted).
    pub unstar: Vec<String>,
    /// The merged star of every id that has local tracks, to apply to them.
    pub local: HashMap<String, bool>,
    /// The merged starred set: the server set once `star`/`unstar` apply.
    pub starred: HashSet<String>,
}

/// Merges one server's starred set with the desktop's stars.
pub(crate) fn merge(input: MergeInput<'_>) -> MergePlan {
    let ids: HashSet<&String> = input
        .base
        .iter()
        .chain(input.server.keys())
        .chain(input.local.keys())
        .collect();
    let mut plan = MergePlan::default();
    for id in ids {
        let server = input.server.get(id).copied();
        let local = input.local.get(id);
        let merged = decide(input.base.contains(id), server, local, input.synced_at_ms);
        match (merged, server.is_some()) {
            (true, false) => plan.star.push(id.clone()),
            (false, true) => plan.unstar.push(id.clone()),
            _ => {}
        }
        if local.is_some() {
            plan.local.insert(id.clone(), merged);
        }
        if merged {
            plan.starred.insert(id.clone());
        }
    }
    plan.star.sort();
    plan.unstar.sort();
    plan
}

/// The merged star of one id.
///
/// `server` is `Some(starred_at)` when the server has it starred. Without
/// local tracks the server is followed as is. When both sides changed and the
/// server starred it, the later action wins; a server unstar carries no time,
/// so a local change made since the last sync wins over it.
fn decide(base: bool, server: Option<i64>, local: Option<&LocalStar>, synced_at_ms: i64) -> bool {
    let server_starred = server.is_some();
    let Some(local) = local else {
        return server_starred;
    };
    let local_changed =
        local.starred != base || local.changed_at_ms.is_some_and(|at| at > synced_at_ms);
    let server_changed = server_starred != base;
    match (local_changed, server_changed) {
        (false, _) => server_starred,
        (true, false) => local.starred,
        (true, true) => match server {
            Some(starred_at) => {
                let local_at = local.changed_at_ms.unwrap_or(i64::MIN);
                if local_at > starred_at.saturating_mul(1000) {
                    local.starred
                } else {
                    true
                }
            }
            None => local.starred,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(items: &[&str]) -> HashSet<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    fn server(items: &[(&str, i64)]) -> HashMap<String, i64> {
        items
            .iter()
            .map(|(id, at)| ((*id).to_string(), *at))
            .collect()
    }

    fn local(items: &[(&str, bool, Option<i64>)]) -> HashMap<String, LocalStar> {
        items
            .iter()
            .map(|(id, starred, at)| ((*id).to_string(), LocalStar::new(*starred, *at)))
            .collect()
    }

    fn run(
        base: &[&str],
        synced_at_ms: i64,
        server_set: &[(&str, i64)],
        local_set: &[(&str, bool, Option<i64>)],
    ) -> MergePlan {
        merge(MergeInput {
            base: &ids(base),
            synced_at_ms,
            server: &server(server_set),
            local: &local(local_set),
        })
    }

    #[test]
    fn first_sync_is_a_union() {
        let plan = run(
            &[],
            0,
            &[("s", 5)],
            &[("l", true, None), ("s", false, None)],
        );
        assert_eq!(plan.star, vec!["l".to_string()]);
        assert!(plan.unstar.is_empty());
        assert_eq!(plan.local.get("s"), Some(&true));
        assert_eq!(plan.local.get("l"), Some(&true));
        assert_eq!(plan.starred, ids(&["l", "s"]));
    }

    #[test]
    fn local_changes_are_pushed() {
        let plan = run(
            &["a", "b"],
            1_000,
            &[("a", 1), ("b", 1)],
            &[
                ("a", false, Some(2_000)),
                ("b", true, None),
                ("c", true, Some(3_000)),
            ],
        );
        assert_eq!(plan.star, vec!["c".to_string()]);
        assert_eq!(plan.unstar, vec!["a".to_string()]);
        assert_eq!(plan.starred, ids(&["b", "c"]));
    }

    #[test]
    fn server_changes_are_applied_locally() {
        let plan = run(
            &["a"],
            1_000,
            &[("b", 2)],
            &[("a", true, Some(500)), ("b", false, None)],
        );
        assert!(plan.star.is_empty() && plan.unstar.is_empty());
        assert_eq!(plan.local.get("a"), Some(&false));
        assert_eq!(plan.local.get("b"), Some(&true));
        assert_eq!(plan.starred, ids(&["b"]));
    }

    #[test]
    fn ids_without_local_tracks_follow_the_server() {
        let plan = run(&["gone"], 1_000, &[("new", 2)], &[]);
        assert!(plan.star.is_empty() && plan.unstar.is_empty());
        assert!(plan.local.is_empty());
        assert_eq!(plan.starred, ids(&["new"]));
    }

    #[test]
    fn desktop_unstar_after_a_server_star_wins() {
        // Starred on the server at t=10s; on the desktop the user starred and
        // then unstarred it at t=20s, so it ended unstarred, as in the base.
        let plan = run(&[], 1_000, &[("x", 10)], &[("x", false, Some(20_000))]);
        assert_eq!(plan.unstar, vec!["x".to_string()]);
        assert_eq!(plan.local.get("x"), Some(&false));
        assert!(plan.starred.is_empty());
    }

    #[test]
    fn server_star_after_a_desktop_unstar_wins() {
        let plan = run(&[], 1_000, &[("x", 30)], &[("x", false, Some(20_000))]);
        assert!(plan.unstar.is_empty());
        assert_eq!(plan.local.get("x"), Some(&true));
        assert_eq!(plan.starred, ids(&["x"]));
    }

    #[test]
    fn a_local_restar_beats_an_untimed_server_unstar() {
        // Base starred; the server dropped it; the desktop toggled it off and
        // on again since the last sync.
        let plan = run(&["x"], 1_000, &[], &[("x", true, Some(2_000))]);
        assert_eq!(plan.star, vec!["x".to_string()]);
        assert_eq!(plan.starred, ids(&["x"]));
    }

    #[test]
    fn both_sides_making_the_same_change_needs_no_push() {
        let plan = run(&[], 1_000, &[("x", 3)], &[("x", true, Some(2_000))]);
        assert!(plan.star.is_empty() && plan.unstar.is_empty());
        assert_eq!(plan.starred, ids(&["x"]));
    }

    #[test]
    fn the_most_recent_track_of_a_group_speaks_for_it() {
        let mut group = LocalStar::new(true, Some(100));
        group.add(false, Some(200));
        assert_eq!(group, LocalStar::new(false, Some(200)));
        group.add(true, Some(150));
        assert_eq!(group, LocalStar::new(false, Some(200)));
        group.add(true, Some(200));
        assert!(group.starred, "a tie goes to the star");
        let mut unstamped = LocalStar::new(false, None);
        unstamped.add(true, None);
        assert!(unstamped.starred);
    }
}
