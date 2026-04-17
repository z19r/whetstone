#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="$ROOT_DIR/VERSION"

usage() {
    cat <<'USAGE'
Usage:
  just release-publish patch
  just release-publish minor
  just release-publish major
  just release-publish set 1.2.3

What it does:
  1) bump VERSION (and create local tag)
  2) commit VERSION
  3) push commit
  4) push tag
USAGE
}

if [[ ! -f "$VERSION_FILE" ]]; then
    echo "VERSION file not found at $VERSION_FILE" >&2
    exit 1
fi

if [[ $# -lt 1 ]]; then
    usage
    exit 1
fi

mode="$1"
if [[ "$mode" == "-h" || "$mode" == "--help" || "$mode" == "help" ]]; then
    usage
    exit 0
fi

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "release-publish must run in a git repo" >&2
    exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
    echo "Working tree is not clean. Commit or stash first." >&2
    git status --short
    exit 1
fi

bash "$ROOT_DIR/scripts/release.sh" "$@" --tag

new_version="$(tr -d '[:space:]' < "$VERSION_FILE")"
tag="v$new_version"

git add VERSION
git commit -m "release: $tag"
git push origin HEAD
git push origin "$tag"

echo "Published release $tag"
