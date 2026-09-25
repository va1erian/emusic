#!/usr/bin/env bash
# usage: bump-win32ui.sh [<commit>]
# Points Cargo.lock's win32ui git dependency at <commit> (default: the tip of
# win32ui's main) and nothing else.
#
# `cargo update -p win32ui` also unlocks the crates win32ui shares with the
# rest of the graph (the `windows` family), and re-resolving them can move
# gpu-allocator off the `windows` version wgpu-hal needs, breaking the build.
# Rewriting only the pinned commit keeps every other lock entry as it was;
# `cargo metadata --locked` then checks the lockfile is still consistent.
set -euo pipefail
REPO=$(git rev-parse --show-toplevel)
URL=https://github.com/va1erian/win32ui
NEW=${1:-$(git ls-remote "$URL" refs/heads/main | cut -f1)}
[[ $NEW =~ ^[0-9a-f]{40}$ ]] || { echo "not a full commit hash: $NEW" >&2; exit 1; }
LOCK="$REPO/Cargo.lock"
OLD=$(sed -n "s|^source = \"git+$URL?branch=main#\([0-9a-f]\{40\}\)\"$|\1|p" "$LOCK" | head -n1)
[ -n "$OLD" ] || { echo "win32ui git source not found in Cargo.lock" >&2; exit 1; }
if [ "$OLD" = "$NEW" ]; then echo "win32ui already at $NEW"; exit 0; fi
sed -i "s|$URL?branch=main#$OLD|$URL?branch=main#$NEW|" "$LOCK"
cargo metadata --locked --format-version 1 --manifest-path "$REPO/Cargo.toml" >/dev/null
echo "win32ui ${OLD:0:8} -> ${NEW:0:8}"
