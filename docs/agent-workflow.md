# How emusic is built: agent workflow

Almost all of emusic is written by AI agents working from GitHub issues, one issue per pull request. This document records the process and the two scripts in `scripts/` that automate it, so a new session (or a new person) can pick it up.

## The loop

1. **An issue is the unit of work.** Each is self-contained: what to build, where the code goes, how to verify it, and a pointer to [AGENTS.md](../AGENTS.md). If an issue needs a product decision, it says so and waits rather than guessing. The overall plan lives in issue #1.
2. **Dispatch** one issue to an agent with `scripts/dispatch.sh` (OpenCode) or to a Claude subagent for cross-cutting work.
3. **The agent** implements it, runs every check, commits, rebases on `main`, pushes and opens a PR. It never merges.
4. **Review** the diff: scope, file sizes, `unsafe`, `unwrap()` outside tests, and — for UI work — the rendered screenshots. Agents are unreliable judges of their own UI output; look at the PNG yourself.
5. **Land** with `scripts/land.sh <pr>`: rebase, run the checks locally, push, wait for CI, rebase-merge, remove the worktree.

## Rules that keep parallel agents from colliding

- **One worktree per issue**, so agents never share a checkout. All of them share **one** `CARGO_TARGET_DIR`: a per-worktree `target/` is 5–11 GB and filled a 466 GB drive once.
- **Rebase only, never merge.** `main` requires linear history, a green `ci` check and an up-to-date branch, for admins too. `land.sh` resolves `Cargo.lock` conflicts automatically (take `main`'s, regenerate) and stops for anything else.
- **Tell each agent which files another agent is touching.** Most conflicts came from two agents editing the app crate at once.
- **At most two concurrent OpenCode runs.** More just gets throttled by the provider, and runs sit idle in API calls.
- **`land.sh` never force-pushes over a branch that moved.** It skips instead, because an agent may have pushed a newer version while the landing was in flight.

## The scripts

### `scripts/dispatch.sh` — run an agent on an issue

```bash
scripts/dispatch.sh <issue> <slug> <model> "<extra instructions>"
```

Creates the worktree and branch (`feat/<issue>-<slug>`), runs the agent with a standard prompt (read the issue and AGENTS.md, implement, run the four checks, commit, rebase, push, open the PR), and **retries up to three times** if a run ends without a PR, telling the next attempt to inspect `git status` and finish what's there. Logs go to `oc-issue-<n>.log`.

Useful variables: `WT=`, `BR=`, `RETRIES=`, `RESUME_FIRST=1` (start with the "finish the existing work" prompt, for resuming an interrupted run).

The extra instructions matter more than the model. Say which files to touch, which files another agent owns, and how to verify (screenshots, real DLLs, temp folders).

### `scripts/land.sh` — review-approved PR to `main`

```bash
scripts/land.sh <pr> [<pr>...]
```

Per PR: rebase on `origin/main`, auto-resolve `Cargo.lock` only, run `fmt`/`check`/`clippy`/`test` (including `--features emusic/shot`), push with lease, wait for CI to appear **and** finish, then rebase-merge and clean up. It prints `PR <n>: MERGED` or the reason it stopped.

## Choosing a model

| Work | Use |
|---|---|
| Self-contained crate or view, bug fix with a clear diagnosis | OpenCode `deepseek-v4.1-flash` — one-shot on most issues here |
| Fallback when deepseek struggles | OpenCode `kimi-k2.7-code` |
| Cross-cutting wiring, anything needing the real app run and observed, conflict-heavy rebases | A Claude subagent |

Avoid `glm-5.3`: it repeatedly ended runs after only reading files.

## Verification that actually caught bugs

- **Real BASS DLLs** (`EMUSIC_BASS_DIR`) found a parallel-init failure and wrong codec names that a DLL-less sandbox could not.
- **The official `bass.h`** caught two wrong tag constants that were written from memory.
- **Headless screenshots** (`emusic-shot`) caught a centred-instead-of-left-aligned column browser and an off-centre play icon.
- **Running the real scanner against a network drive** proved the scanner was fine and pointed at the actual bug: the UI thread waiting on a database lock held for the whole scan.

The pattern: whenever an agent claims something works, try to reproduce the user-visible behaviour with the real thing.
