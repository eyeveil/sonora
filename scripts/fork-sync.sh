#!/usr/bin/env bash
# Rebases the fork's `custom` branch onto upstream (default: the latest release, upstream/stable),
# runs the checks in the dev shell and force-pushes it to origin. FORK.md explains the fork.
set -euo pipefail

base=${1:-upstream/stable}
cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "fork-sync: the working tree has changes; commit or stash them first" >&2
  exit 1
fi

git config rerere.enabled true
git fetch upstream --tags
git fetch origin
git checkout custom
# After a conflict was resolved, local custom is already ahead of origin and must stay so.
if git merge-base --is-ancestor HEAD origin/custom; then
  git merge --ff-only origin/custom
fi

if ! git rebase "$base"; then
  echo "fork-sync: resolve the conflicts, run 'git rebase --continue', then run this again" >&2
  exit 1
fi

if [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/custom)" ]; then
  echo "fork-sync: custom already sits on $base"
  exit 0
fi

nix develop --quiet -c bash -c 'cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace'
git push --force-with-lease origin custom
echo "fork-sync: custom is now $(git rev-parse --short HEAD) on $base"
echo "fork-sync: next, cd ~/conf && nix flake update sonora, then build and push ~/conf"
