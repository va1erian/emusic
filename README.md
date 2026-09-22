# emusic

Windows music player & library, built with Rust + egui, audio via [BASS](https://www.un4seen.com).

## Development

Requirements: a current stable Rust toolchain (see `rust-toolchain.toml`).

Build and run:

```
cargo run -p emusic
```

Checks used in CI:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### BASS DLLs

Audio playback uses the native BASS library. You must download the BASS DLLs (x64) yourself from <https://www.un4seen.com> and place them in a `bass/` directory next to the built executable:

- `bass.dll`
- `bassflac.dll`
- `bassopus.dll`
- `bassalac.dll`
- `basswv.dll`
- `bassape.dll`
- `bass_mpc.dll`
- `basswebm.dll`

The DLLs are never committed to this repository. The directory defaults to `<exe dir>/bass/` and can be overridden with the `EMUSIC_BASS_DIR` environment variable.

Note: BASS is free for non-commercial use; see the license on the un4seen website.

## License

MIT — see [LICENSE](LICENSE). BASS is third-party software with its own license.
