use serde::{Deserialize, Serialize};

/// Identifier of a [`crate::Track`], backed by the SQLite `tracks.id` row id.
///
/// A `TrackId` of `0` is never assigned by the store (SQLite row ids start
/// at 1), so it is used by callers as an "unassigned" sentinel before a
/// track has been persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TrackId(pub i64);

impl TrackId {
    /// Sentinel value for a track that has not yet been assigned an id by
    /// the store (e.g. freshly scanned, not-yet-persisted tracks).
    pub const UNASSIGNED: TrackId = TrackId(0);

    /// Returns `true` if this id has not been assigned by the store yet.
    pub fn is_unassigned(self) -> bool {
        self == Self::UNASSIGNED
    }
}

impl From<i64> for TrackId {
    fn from(value: i64) -> Self {
        TrackId(value)
    }
}

impl From<TrackId> for i64 {
    fn from(value: TrackId) -> Self {
        value.0
    }
}

impl std::fmt::Display for TrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unassigned_sentinel() {
        assert!(TrackId::UNASSIGNED.is_unassigned());
        assert!(!TrackId(1).is_unassigned());
    }

    #[test]
    fn conversions_round_trip() {
        let id: TrackId = 42.into();
        assert_eq!(id, TrackId(42));
        let raw: i64 = id.into();
        assert_eq!(raw, 42);
    }

    #[test]
    fn display_matches_inner_value() {
        assert_eq!(TrackId(7).to_string(), "7");
    }
}
