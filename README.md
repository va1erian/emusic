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

Audio playback uses the native BASS library. You must download the BASS DLLs (x64) yourself from <https://www.un4seen.com> and place them in a `bass/` directory next to the built executable. `bass.dll` (the audio engine) is required; the codec add-ons are optional, and the app loads one for every other `bass*.dll` it finds in that directory (`crates/bass/src/ffi/loader.rs`):

- `bass.dll` (required)
- `bassflac.dll`
- `bassopus.dll`
- `bassmidi.dll`
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

The installer needs these DLLs at packaging time and fails to compile if the `bass/` folder is missing or empty — see [docs/installer.md](docs/installer.md).

### Icons

The app's icon set lives in `assets/` (`ico/`, `png/`, `svg/` — see
`assets/README.txt`). Unlike the BASS DLLs it is committed. The app icon is
embedded into `emusic.exe` at build time (via `crates/app/build.rs` and
`embed-resource`) and set as the egui window icon; the per-extension
`file-<ext>.ico` files ship in an `icons/` folder next to the installed exe
and back the file-association `DefaultIcon` entries. The installer packages
them — see [docs/installer.md](docs/installer.md).

### Fonts

The app ships **no bundled fonts**: it uses the fonts already installed on the machine (`C:\Windows\Fonts`), with Segoe UI as the primary UI font and the CJK/symbol/emoji fonts as fallbacks. If none of them are found (e.g. a non-Windows dev box) it logs a warning and falls back to egui's defaults.

## Packaging (Windows installer)

A per-user installer is built from `installer/emusic.iss` with [Inno Setup 7](https://jrsoftware.org/isdl.php): `cargo build --release`, place the BASS x64 DLLs in `target\release\bass\` (a required input — the compile fails without them), then compile the script with `ISCC.exe`. A separate `installer/emusic-win32.iss` packages the native Win32 frontend (`emusic-win32.exe`) as its own installer, side by side with the egui build under a different app name and install dir. See [docs/installer.md](docs/installer.md) for the full steps, including how the two share config/library data but not file associations.

### Downloads

Each [GitHub release](https://github.com/va1erian/emusic/releases) ships four assets, built by `.github/workflows/release.yml` on every `v*` tag: an installer and a portable zip for each frontend (`emusic-<version>-setup.exe` / `emusic-<version>-portable.zip` for egui, `emusic-win32-<version>-setup.exe` / `emusic-win32-<version>-portable.zip` for the native Win32 build). Both frontends can be installed at once; they read the same library.

## License

emusic's own code is MIT — see [LICENSE](LICENSE).

Audio playback uses the third-party [BASS](https://www.un4seen.com) library by un4seen developments, which is **free for non-commercial use**. emusic is personal freeware, so it is covered by those terms; the BASS DLLs ship with the installer under that license and are never committed to git. emusic's MIT license does not change BASS's terms: a **commercial** fork or build would need its own BASS license from un4seen. See the `bass.txt` license files bundled with the BASS download.

## How this project is built

Almost all of emusic is written by AI agents working from GitHub issues, one issue per PR. The process and the dispatch/landing scripts are documented in [docs/agent-workflow.md](docs/agent-workflow.md); the rules agents must follow are in [AGENTS.md](AGENTS.md).
