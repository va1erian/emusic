# Test fixtures

## SID tunes

`crates/player/tests/sid_integration.rs` plays real Commodore 64 tunes to
verify that the cRSID engine renders audible PCM and that switching subtunes
changes the output. The tunes are bundled under
`crates/player/tests/fixtures/sid/` so the tests run in CI without any BASS
install.

| File | Title | Author | Subtunes |
| --- | --- | --- | --- |
| `Winners.sid` | Winners | Mitch & Dane | 1 |
| `Mini_Melodies_Compilation.sid` | Mini Melodies Compilation | 4-Mat | 6 |

Both tunes are part of the [High Voltage SID Collection](https://hvsc.c64.org)
(HVSC) and remain © their respective composers. They are included here solely
as test fixtures for the SID playback and subtune-switching tests; emusic's
MIT licence does not apply to them.

The tests still skip gracefully if a fixture is removed: point
`EMUSIC_SID_TEST_FILE` / `EMUSIC_SID_TEST_MULTI` at another tune, or delete the
file. The BASS-backed channel test additionally needs `bass.dll` (in
`EMUSIC_BASS_DIR` or `<exe dir>/bass/`) and skips when it is absent — the
engine tests need no audio device and render PCM into memory only.
