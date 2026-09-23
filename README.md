# emusic

Windows music player & library, built with Rust + egui, audio via [BASS](https://www.un4seen.com).

## Development

Requirements: a current stable Rust toolchain (see `rust-toolchain.toml`).

Build and run:

```
cargo run -p emusic
```

Checks used in CI (all must pass before submitting; see AGENTS.md):

```
cargo fmt --all --check
cargo check --workspace --all-targets
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

The DLLs are never committed to this repository. The directory defaults to `<exe dir>/bass/` and can be overridden with the `EMUSIC_BASS_DIR` environment variable:

```sh
set EMUSIC_BASS_DIR=C:\path\to\BASS\x64
```

Note that each Windows BASS download zip contains both 32-bit and 64-bit builds — the x64 DLLs live in the `x64/` subfolder, so point the player (or `EMUSIC_BASS_DIR`, e.g. for `cargo test -p bass`) at that subfolder, not the zip root.

Note: BASS is free for non-commercial use; see the license on the un4seen website.

### Fonts

The app ships **no bundled fonts**: it uses the fonts already installed on the machine (`C:\Windows\Fonts`), with Segoe UI as the primary UI font and the CJK/symbol/emoji fonts as fallbacks. If none of them are found (e.g. a non-Windows dev box) it logs a warning and falls back to egui's defaults.

## Packaging (Windows installer)

A per-user installer is built from `installer/emusic.iss` with [Inno Setup 7](https://jrsoftware.org/isdl.php): `cargo build --release`, place the BASS x64 DLLs in `target\release\bass\`, then compile the script with `ISCC.exe`. See [docs/installer.md](docs/installer.md) for the full steps.

## License

MIT — see [LICENSE](LICENSE). BASS is third-party software with its own license.

## How this project is built

Almost all of emusic is written by AI agents working from GitHub issues, one issue per PR. The process and the dispatch/landing scripts are documented in [docs/agent-workflow.md](docs/agent-workflow.md); the rules agents must follow are in [AGENTS.md](AGENTS.md).
