//! Unit tests for the album tile placeholder. The ordering and identity
//! tests moved to `emusic-ui` with the catalog in #93.

use super::tile;

#[test]
fn placeholder_colour_is_stable_per_album_name() {
    assert_eq!(
        tile::placeholder_color("Echoes"),
        tile::placeholder_color("Echoes")
    );
    assert_ne!(
        tile::placeholder_color("Echoes"),
        tile::placeholder_color("Voyage")
    );
}
