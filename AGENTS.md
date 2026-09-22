# Agent & contributor guidelines

emusic is a Windows music player & library in Rust + egui, audio via BASS. The full plan is issue #1; each task is a GitHub issue.

## Code quality
- Keep code clean, structured and readable. Prefer small, focused modules: **aim for files under ~300 lines, hard limit ~500**. Split by responsibility (e.g. `view/`, `widgets/`, `ffi/`), not arbitrarily.
- Clear names, doc comments on public items, no dead code, no commented-out code, no `println!` debugging (use `tracing`).
- Errors: `thiserror` in library crates, `anyhow` only in the `app` binary. No `unwrap()`/`expect()` outside tests except for true invariants (with a message explaining why).
- Match the existing style of the surrounding code.

## Unsafe
- **Avoid `unsafe`.** Every crate except `bass` and `winshell` must have `#![forbid(unsafe_code)]` in its `lib.rs`/`main.rs`.
- In `bass` and `winshell`, `unsafe` lives only in a small dedicated module (`ffi`/`sys`) wrapped by a safe API. Every `unsafe` block gets a `// SAFETY:` comment. Prefer safe crates where they exist (e.g. `winreg` for the registry).

## Workspace rules
- Edition 2024, `members = ["crates/*"]`. Declare dependencies in **your crate's own** `Cargo.toml`; do not edit `[workspace.dependencies]` or other crates' manifests unless the issue says so.
- BASS DLLs are never committed. Loaded at runtime from `<exe dir>/bass/` (override: env `EMUSIC_BASS_DIR`). Tests that need BASS must skip gracefully when the DLLs are absent.
- Stay within the issue's scope; list follow-ups in the PR description.

## Before submitting (mandatory)
All of these must pass locally — the same checks CI runs:

```
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Do not submit work with failing or skipped checks. PR bodies must contain `Closes #<issue>`.
