#!/usr/bin/env bash
# Whetstone remote installer — curl -fsSL URL | bash
# Stays in your current directory (your git project); clones repo to
# ~/.whetstone for scripts + bin, then runs setup there.
set -euo pipefail

DEFAULT_REPO="https://github.com/z19r/whetstone.git"
REPO="${WHETSTONE_REPO:-$DEFAULT_REPO}"
HOME_DIR="${WHETSTONE_HOME:-$HOME/.whetstone}"
VERSION_FILE="$HOME_DIR/VERSION"

read_local_version() {
    if [[ -f "$VERSION_FILE" ]]; then
        tr -d '[:space:]' < "$VERSION_FILE"
    else
        echo ""
    fi
}

echo "Whetstone remote install"
echo "  repo: $REPO"
echo "  copy: $HOME_DIR"
echo "  project cwd: $(pwd)"
echo ""

if [[ ! -d "$HOME_DIR/.git" ]]; then
    git clone "$REPO" "$HOME_DIR"
else
    old_version="$(read_local_version)"
    git -C "$HOME_DIR" pull --ff-only || \
        echo "[Whetstone] git pull failed; using existing tree" >&2
    new_version="$(read_local_version)"
    if [[ -n "$old_version" && -n "$new_version" ]]; then
        if [[ "$old_version" != "$new_version" ]]; then
            echo "[Whetstone] updated: $old_version -> $new_version"
        else
            echo "[Whetstone] already current at version $new_version"
        fi
    fi
fi

if [[ -f "$VERSION_FILE" ]]; then
    echo "[Whetstone] installer version: $(read_local_version)"
fi

export WHETSTONE_ROOT="$HOME_DIR"
exec bash "$HOME_DIR/setup-whetstone.sh" "$@"
