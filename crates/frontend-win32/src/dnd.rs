//! The app-private drag-and-drop payload (#476).
//!
//! Track tables start a drag with the selected track ids encoded here and
//! dropped onto the navigator's playlist rows. `win32ui`'s `begin_drag` only
//! moves an opaque byte blob, so the two ends must agree on a format; the
//! magic prefix keeps the decoder from treating another app's payload as ours.

/// Marks a payload as a list of track ids, so a stray payload is rejected.
const MAGIC: &[u8] = b"emusic-tracks\0";

/// Encodes `tracks` as the drag payload accepted by
/// [`decode_track_ids`].
pub fn encode_track_ids(tracks: &[u64]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(MAGIC.len() + tracks.len() * 8);
    bytes.extend_from_slice(MAGIC);
    for track in tracks {
        bytes.extend_from_slice(&track.to_le_bytes());
    }
    bytes
}

/// Reads the track ids from a drag payload, or `None` when the payload was
/// not produced by [`encode_track_ids`].
pub fn decode_track_ids(payload: &[u8]) -> Option<Vec<u64>> {
    let ids = payload.strip_prefix(MAGIC)?;
    let chunks = ids.as_chunks::<8>().0;
    if ids.len() != chunks.len() * 8 {
        return None;
    }
    Some(
        chunks
            .iter()
            .map(|bytes| u64::from_le_bytes(*bytes))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        let ids = vec![1, 2, 9_999_999, u64::MAX];
        let payload = encode_track_ids(&ids);
        assert_eq!(decode_track_ids(&payload).as_deref(), Some(ids.as_slice()));
    }

    #[test]
    fn empty_and_foreign_payloads_are_rejected() {
        assert_eq!(decode_track_ids(&[]), None);
        assert_eq!(decode_track_ids(b"something else"), None);
        // The magic alone is a valid, empty id list.
        assert_eq!(decode_track_ids(MAGIC), Some(Vec::new()));
        // A truncated id is rejected.
        let mut payload = encode_track_ids(&[7]);
        payload.pop();
        assert_eq!(decode_track_ids(&payload), None);
    }
}
