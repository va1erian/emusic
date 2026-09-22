#!/usr/bin/env bash
# usage: land.sh <pr-number>...
# For each PR in order: rebase onto origin/main (auto-resolving Cargo.lock only),
# run the four checks, force-push with lease, wait for CI, rebase-merge.
set -uo pipefail
REPO=${REPO:-$(git rev-parse --show-toplevel)}
WT_ROOT=${WT_ROOT:-"$REPO/../emusic-wt"}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$REPO/../emusic-target"}
R=va1erian/emusic
for PR in "$@"; do
  BR=$(gh pr view "$PR" -R $R --json headRefName -q .headRefName)
  WT="$WT_ROOT/land-$PR"
  git -C "$REPO" fetch -q origin
  git -C "$REPO" worktree remove --force "$WT" 2>/dev/null
  git -C "$REPO" worktree add -q --detach "$WT" "origin/$BR" || { echo "PR $PR: worktree failed"; continue; }
  cd "$WT"
  if ! git rebase -q origin/main 2>/dev/null; then
    while git status | grep -q "rebase in progress"; do
      conflicts=$(git diff --name-only --diff-filter=U)
      if [ "$conflicts" != "Cargo.lock" ]; then
        echo "PR $PR: CONFLICT needs manual resolution: $conflicts"; git rebase --abort; cd /; continue 2
      fi
      git checkout origin/main -- Cargo.lock && cargo check --workspace -q && git add Cargo.lock
      GIT_EDITOR=true git rebase --continue >/dev/null 2>&1 || true
    done
  fi
  if ! { cargo fmt --all --check && cargo check --workspace --all-targets -q && cargo clippy --workspace --all-targets -q -- -D warnings && cargo clippy --workspace --all-targets -q --features emusic/shot -- -D warnings && cargo test --workspace -q >/dev/null 2>&1 && cargo test --workspace -q --features emusic/shot >/dev/null 2>&1; }; then
    echo "PR $PR: local checks FAILED after rebase"; cd /; continue
  fi
  if ! git push -q --force-with-lease=$BR:origin/$BR origin HEAD:$BR; then echo "PR $PR: branch moved during landing (lease failed), skipped"; cd /; continue; fi
  sha=$(git rev-parse HEAD); until gh pr view "$PR" -R $R --json headRefOid -q .headRefOid | grep -q "$sha" && gh pr checks "$PR" -R $R 2>/dev/null | grep -q .; do sleep 15; done
  gh pr checks "$PR" -R $R --watch --interval 20 >/dev/null 2>&1
  if gh pr checks "$PR" -R $R 2>/dev/null | grep -q fail; then echo "PR $PR: CI FAILED"; cd /; continue; fi
  gh pr merge "$PR" -R $R --rebase >/dev/null 2>&1 && echo "PR $PR: MERGED" || echo "PR $PR: merge refused: $(gh pr view $PR -R $R --json mergeStateStatus -q .mergeStateStatus)"
  cd /; git -C "$REPO" worktree remove --force "$WT" 2>/dev/null
done
git -C "$REPO" pull -q --rebase 2>/dev/null
