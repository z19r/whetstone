#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="$ROOT_DIR/VERSION"

usage() {
    cat <<'USAGE'
Usage:
  just release patch
  just release minor
  just release major
  just release set 1.2.3
  just release patch --tag

Notes:
  - Updates VERSION in-place.
  - Use --tag to create a local git tag (vX.Y.Z).
USAGE
}

extract_semver() {
    local raw="$1"
    echo "$raw" | sed -nE 's/.*([0-9]+\.[0-9]+\.[0-9]+).*/\1/p'
}

bump_version() {
    local current="$1"
    local kind="$2"
    local major minor patch
    major="$(echo "$current" | cut -d. -f1)"
    minor="$(echo "$current" | cut -d. -f2)"
    patch="$(echo "$current" | cut -d. -f3)"

    case "$kind" in
        patch) patch=$((patch + 1)) ;;
        minor) minor=$((minor + 1)); patch=0 ;;
        major) major=$((major + 1)); minor=0; patch=0 ;;
        *) return 1 ;;
    esac

    echo "$major.$minor.$patch"
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
shift

if [[ "$mode" == "-h" || "$mode" == "--help" || "$mode" == "help" ]]; then
    usage
    exit 0
fi

create_tag=0
target_version=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tag)
            create_tag=1
            ;;
        *)
            target_version="$1"
            ;;
    esac
    shift
done

current_raw="$(tr -d '[:space:]' < "$VERSION_FILE")"
current_version="$(extract_semver "$current_raw")"

if [[ -z "$current_version" ]]; then
    echo "Current VERSION is invalid: $current_raw" >&2
    exit 1
fi

if [[ "$mode" == "set" ]]; then
    if [[ -z "$target_version" ]]; then
        echo "set requires an explicit version, e.g. set 1.2.3" >&2
        exit 1
    fi
    new_version="$(extract_semver "$target_version")"
else
    new_version="$(bump_version "$current_version" "$mode" || true)"
fi

if [[ -z "$new_version" ]]; then
    echo "Invalid release mode: $mode" >&2
    usage
    exit 1
fi

echo "$new_version" > "$VERSION_FILE"
echo "Updated VERSION: $current_version -> $new_version"

if [[ "$create_tag" -eq 1 ]]; then
    tag="v$new_version"
    if git rev-parse "$tag" >/dev/null 2>&1; then
        echo "Tag already exists: $tag" >&2
        exit 1
    fi
    git tag "$tag"
    echo "Created tag: $tag"
    echo "Push tag with: git push origin $tag"
fi
