# Frontends

emusic's UI is split from its logic and from the platform it runs on:

- **`crates/ui` (`emusic-ui`)** — the toolkit-agnostic core: `Shell`, `AppState`,
  view models, the library/player/audio seams, config and the `Startup` type. It
  names no windowing toolkit.
- **A frontend crate** — the view layer. It owns the window and the message loop,
  renders the shared state, and runs the shared `Shell`.

The binary (`crates/app`) owns the CLI, the file associations and the
single-instance handshake, then hands a `Startup` to the selected frontend.

## The contract

A frontend is any crate exposing:

```rust
pub fn run(startup: emusic_ui::startup::Startup) -> anyhow::Result<()>
```

That signature is spelled `emusic_ui::startup::RunFn`. The binary stores the
chosen frontend in a `const` of that type, so a frontend whose `run` drifts from
the contract fails to compile.

`Startup` carries everything the frontend needs without it reaching back into
the binary: the loaded `Config`, where to persist it, the IPC bridge (when this
process is the primary instance), startup files, the worker `WakerSlot`, and the
`mock` flag.

## The frontends

- **`crates/frontend-win32`** — the native Win32 renderer built on `win32ui`:
  native common controls, the extended title bar, the strip menu, and the
  Windows shell integrations (SMTC, taskbar thumb-bar, taskbar preview). The
  crate is `#![cfg(windows)]` and compiles to an empty library elsewhere.
- **`crates/frontend-portable`** — the cross-platform renderer built on the
  `xui_core` widget layer: portable widgets for every view, and window chrome on
  the native `win32` backend (Windows) or the software `canvas` backend
  elsewhere.
- **`crates/platform` (`emusic-platform`)** — the OS shell integrations both
  renderers share: the `ShellIntegration` seam, and its Windows (SMTC, taskbar
  thumb-bar, file associations) and macOS (Now Playing) implementations. The
  portable renderer talks to the OS only through this crate, so no view names a
  platform type.

## Selecting a frontend

Selection is by target, with a Windows-only override:

| Target | Default frontend | Override |
|---|---|---|
| Windows | `frontend-win32` | `--features emusic/portable` picks `frontend-portable` |
| non-Windows | `frontend-portable` | — |

```powershell
cargo run -p emusic                                     # native on Windows
cargo run -p emusic --features portable                 # portable frontend
$env:XUI_BACKEND = 'canvas'; cargo run -p emusic --features portable
```

The `emusic/portable` feature is for development and backend comparison on
Windows. Inside the portable frontend, `XUI_BACKEND=canvas` chooses the software
backend over the native Win32 backend.

## Adding a frontend

1. Create a crate exposing `pub fn run(Startup) -> anyhow::Result<()>`, driving
   `emusic_ui::shell::Shell` and rendering `AppState`.
2. Add it to `crates/app`'s target-specific dependencies and extend the
   `cfg`/feature ladder in `crates/app/src/main.rs`.
3. Keep it thin: logic and state live in `emusic-ui`; only views and OS services
   belong in the frontend.
