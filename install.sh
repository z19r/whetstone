#!/usr/bin/env bash
# Whetstone remote installer — curl -fsSL URL | bash
# Stays in your current directory (your git project); clones repo to
# ~/.whetstone for scripts + bin, then runs setup there.
set -euo pipefail

DEFAULT_REPO="https://github.com/z19r/whetstone.git"
REPO="${WHETSTONE_REPO:-$DEFAULT_REPO}"
HOME_DIR="${WHETSTONE_HOME:-$HOME/.whetstone}"

echo "Whetstone remote install"
echo "  repo: $REPO"
echo "  copy: $HOME_DIR"
echo "  project cwd: $(pwd)"
echo ""

if [[ ! -d "$HOME_DIR/.git" ]]; then
    git clone "$REPO" "$HOME_DIR"
else
    git -C "$HOME_DIR" pull --ff-only || \
        echo "[Whetstone] git pull failed; using existing tree" >&2
fi

export WHETSTONE_ROOT="$HOME_DIR"
exec bash "$HOME_DIR/setup-whetstone.sh"
