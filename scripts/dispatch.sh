#!/usr/bin/env bash
# usage: [WT=..] [BR=..] [RETRIES=3] [RESUME_FIRST=1] dispatch.sh <issue> <slug> <model> "<extra instructions>"
# <model> is an opencode-go model name, or a full provider/model id (e.g. fireworks-ai/accounts/fireworks/models/deepseek-v4p1-flash).
# Runs OpenCode on an issue in its own worktree; if a run ends without a PR
# for the branch, retries in the same worktree with a "finish the job" prompt.
set -uo pipefail
# The main checkout, even when run from a linked worktree.
REPO=${REPO:-$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")}
WT_ROOT=${WT_ROOT:-"$REPO/../emusic-wt"}
N=$1; SLUG=$2; MODEL=$3; EXTRA=${4:-}
WT=${WT:-"$WT_ROOT/issue-$N"}
BR=${BR:-feat/$N-$SLUG}
RETRIES=${RETRIES:-3}
# Logs live beside the worktrees, not inside the agent's checkout.
mkdir -p "$WT_ROOT/logs"
LOG=${LOG:-"$WT_ROOT/logs/oc-issue-$N.log"}
R=va1erian/emusic

git -C "$REPO" fetch -q origin
if [ ! -d "$WT" ]; then
  git -C "$REPO" worktree add -q "$WT" -b "$BR" origin/main || exit 1
fi
cd "$WT" || exit 1
# Concurrent builds must not share a target dir (cargo fingerprint collisions):
# one per worktree unless CARGO_TARGET_DIR is set explicitly.
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$WT/target"}
export CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-0}
command -v sccache >/dev/null && export RUSTC_WRAPPER=${RUSTC_WRAPPER:-sccache}

pr_url() { gh pr list -R $R --head "$BR" --state all --json url -q '.[0].url' 2>/dev/null; }

TASK="You are working in a git worktree of va1erian/emusic (Windows music player, Rust + egui, audio via BASS). Branch: $BR (based on latest origin/main).
Task: implement GitHub issue #$N. Read it: gh issue view $N -R $R --comments (plan: issue #1). Read AGENTS.md FIRST and follow it strictly: clean readable code, small focused files (<300 lines, split tests into separate files if needed), #![forbid(unsafe_code)], no unwrap outside tests, rebase-only git workflow. Look at existing crates in crates/ and reuse their public APIs (read their lib.rs and docs) rather than duplicating.
$EXTRA
Do NOT end your turn after exploring: keep reading to what you need, then write the code, run the checks and open the PR, all in this same run.
Before submitting, ALL must pass: cargo fmt --all --check ; cargo check --workspace --all-targets ; cargo clippy --workspace --all-targets -- -D warnings ; cargo test --workspace.
Then: commit (message: '<short summary> (#$N)', blank line, 'Closes #$N'), run 'git fetch origin && git rebase origin/main' (on Cargo.lock conflict take origin/main's and re-run cargo check), re-run the four checks, push with 'git push -u origin $BR', and open a PR: gh pr create -R $R -B main -t '<title>' -b '<summary, public API, follow-ups, Closes #$N, and a last line: Implemented by OpenCode ($MODEL)>'. Do NOT merge. Finish by printing the PR URL and a short summary."

RESUME="You are continuing unfinished work on GitHub issue #$N of $R in this git worktree (branch $BR). A previous run ended before opening a pull request. First inspect the current state: 'git status', 'git log --oneline origin/main..HEAD', 'git diff'. Keep any good work already present and finish the job; do not start over unless the existing work is broken.
Original instructions follow.
$TASK"

attempt=0
PROMPT=$TASK
[ "${RESUME_FIRST:-}" = 1 ] && PROMPT=$RESUME
while :; do
  attempt=$((attempt + 1))
  echo "===== attempt $attempt ($(date +%T)) =====" >> "$LOG"
  opencode run --auto -m "$([[ $MODEL == */* ]] && echo "$MODEL" || echo "opencode-go/$MODEL")" --title "emusic #$N (attempt $attempt)" "$PROMPT" >> "$LOG" 2>&1
  url=$(pr_url)
  if [ -n "$url" ]; then
    echo "OK #$N after $attempt attempt(s): $url"
    exit 0
  fi
  if [ "$attempt" -gt "$RETRIES" ]; then
    echo "FAILED #$N: no PR after $attempt attempts (log: $LOG)"
    git status --short | head -20
    exit 1
  fi
  PROMPT=$RESUME
done
