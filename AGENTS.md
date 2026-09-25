# Agent & contributor guidelines

emusic is a Windows music player & library in Rust with a native Win32 UI, audio via BASS. The full plan is issue #1; each task is a GitHub issue.

## Code quality
- Keep code clean, structured and readable. Prefer small, focused modules: **aim for files under ~300 lines, hard limit ~500**. Split by responsibility (e.g. `view/`, `widgets/`, `ffi/`), not arbitrarily.
- Clear names, doc comments on public items, no dead code, no commented-out code, no `println!` debugging (use `tracing`).
- Errors: `thiserror` in library crates, `anyhow` only in the `app` binary. No `unwrap()`/`expect()` outside tests except for true invariants (with a message explaining why).
- Match the existing style of the surrounding code.

## Unsafe
- **Avoid `unsafe`.** Every crate except `bass`, `winshell` and `sid` must have `#![forbid(unsafe_code)]` in its `lib.rs`/`main.rs`.
- In `bass`, `winshell` and `sid`, `unsafe` lives only in a small dedicated module (`ffi`/`sys`) wrapped by a safe API. Every `unsafe` block gets a `// SAFETY:` comment. In `sid`, every module outside `src/ffi.rs` starts with `#![forbid(unsafe_code)]`. Prefer safe crates where they exist (e.g. `windows-registry` for the registry, `interprocess` for named pipes).
- `win32ui` (the safe Win32/common-controls wrapper the app is built on) lives in its own repo, [va1erian/win32ui](https://github.com/va1erian/win32ui), and is pulled in as a git dependency — its `unsafe` is reviewed and tested there, not here. `crates/win32ui-demo` consumes it and holds the workspace's example + smoke test.
- To pick up new win32ui commits, run `scripts/bump-win32ui.sh [<commit>]` — **not** `cargo update -p win32ui`, which re-resolves the shared `windows` crates and can break the build.

## Workspace rules
- Edition 2024, `members = ["crates/*"]`. Declare dependencies in **your crate's own** `Cargo.toml`; do not edit `[workspace.dependencies]` or other crates' manifests unless the issue says so.
- BASS DLLs are never committed. Loaded at runtime from `<exe dir>/bass/` (override: env `EMUSIC_BASS_DIR`). Tests that need BASS must skip gracefully when the DLLs are absent.
- `crates/sid` vendors the cRSID C engine and builds it with a MinGW-w64 GCC via the `cc` crate (cRSID uses GCC nested functions, which MSVC/clang reject); `EMUSIC_SID_CC` overrides the compiler. The vendored sources are unmodified — do not patch them for one toolchain.
- Stay within the issue's scope; list follow-ups in the PR description.

## Git workflow (no merge commits)
- Branch from the latest `origin/main`; one branch per issue (`feat/<issue>-<slug>`).
- **Always rebase, never merge:** before submitting, `git fetch origin && git rebase origin/main`. Use `git pull --rebase` when updating. Never merge `main` into your branch.
- On a `Cargo.lock` conflict, take `origin/main`'s version and re-run `cargo check` to regenerate it.
- After rebasing, re-run all checks below, then push with `--force-with-lease`.
- PRs are integrated into `main` with **rebase merge** only, keeping history linear.

## Before submitting (mandatory)
All of these must pass locally — the same checks CI runs:

```
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Do not submit work with failing or skipped checks. PR bodies must contain `Closes #<issue>`.

## Running tests on a shared desktop: use the Windows Sandbox

The UI tests create real top-level windows, move focus and send synthetic input, and render Direct2D frames. On your own (or another agent's) desktop that steals focus and makes runs flaky, so run the test suite inside Windows Sandbox:

```powershell
scripts\sandbox\run.ps1                                    # every test
scripts\sandbox\run.ps1 -CargoArgs '--features','emusic/shot'
scripts\sandbox\run.ps1 -BassDir C:\BASS\x64               # exercise the real BASS code
```

It builds on the host, stages each binary with its crate's `tests\` folder and the BASS DLLs, then runs everything on the sandbox's own desktop. See [docs/sandbox-testing.md](docs/sandbox-testing.md) for the full options and the known limitations. Plain `cargo test` on the host is only acceptable in CI (already a disposable VM) or where the sandbox is unavailable; `fmt`, `check` and `clippy` don't open windows and stay on the host. **Only one sandbox can run at a time** — if the script reports one already running, it belongs to another agent: wait or report it, never kill it.

## UI changes: use `emusic-shot`

`crates/app` has the screenshot tool (see #118) for looking at the shell without a display:

```
cargo run -p emusic --features shot --bin emusic-shot -- --all --theme dark --out crates/app/docs/screenshots/dark
cargo run -p emusic --features shot --bin emusic-shot -- --view music --theme light --out crates/app/docs/screenshots/light/music.png
```

It runs the real `Win32App` against deterministic mock data (`emusic --mock` runs the same data interactively) and captures the window with `win32ui`'s occlusion-proof `Windows.Graphics.Capture` path (frame, caption buttons and backdrop material included; `PrintWindow` fallback). It renders one fresh process per view, so `--all` is consistent. The committed dark/light shots under `crates/app/docs/screenshots/` are the reference set: for **any** change touching `crates/app` UI code, regenerate the shots for the views you touched, in both themes, and **look at the PNGs** before submitting. Describe or attach the relevant screenshots in the PR.

## Behaviour changes: verify with UI Automation

The app exposes a UI Automation tree, so you can click, type and read state in the real app instead of guessing from screenshots. See [docs/win32-uia.md](docs/win32-uia.md) and `scripts/win32-uia.ps1`. Prefer it for any change that depends on input handling (clicks, keys, focus, menus); use real input (`Click-Uia`, `Send-Key`) rather than direct invokes for those, and always stop the process you start.
