#!/usr/bin/env bash
# Whetstone installer — curl -fsSL URL | bash
# Downloads prebuilt binary from GitHub Releases, installs to ~/.local/bin,
# fetches assets to ~/.whetstone/assets/, then runs `whetstone setup`.
set -euo pipefail

REPO="zackkitzmiller/whetstone"
BIN_DIR="${HOME}/.local/bin"
ASSETS_DIR="${HOME}/.whetstone/assets"

info()  { printf '\033[0;34m[whetstone]\033[0m %s\n' "$1"; }
ok()    { printf '\033[0;32m[whetstone]\033[0m %s\n' "$1"; }
fail()  { printf '\033[0;31m[whetstone]\033[0m %s\n' "$1" >&2; exit 1; }

detect_target() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      fail "unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64)         arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)              fail "unsupported architecture: $arch" ;;
    esac

    echo "${arch}-${os}"
}

TARGET="$(detect_target)"
ARCHIVE="whetstone-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE}"

info "detected target: ${TARGET}"
info "downloading ${DOWNLOAD_URL}"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL -o "${TMPDIR}/${ARCHIVE}" "$DOWNLOAD_URL" \
    || fail "download failed — check https://github.com/${REPO}/releases for available binaries"

info "extracting binary"
tar xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"

mkdir -p "$BIN_DIR"
mv "${TMPDIR}/whetstone" "${BIN_DIR}/whetstone"
chmod +x "${BIN_DIR}/whetstone"
ok "installed binary to ${BIN_DIR}/whetstone"

info "fetching assets"
ASSETS_URL="https://github.com/${REPO}/releases/latest/download/whetstone-assets.tar.gz"
if curl -fsSL -o "${TMPDIR}/assets.tar.gz" "$ASSETS_URL" 2>/dev/null; then
    mkdir -p "$ASSETS_DIR"
    tar xzf "${TMPDIR}/assets.tar.gz" -C "$ASSETS_DIR"
    ok "installed assets to ${ASSETS_DIR}"
else
    info "assets archive not found; cloning from repo"
    git clone --depth 1 --filter=blob:none --sparse \
        "https://github.com/${REPO}.git" "${TMPDIR}/repo" 2>/dev/null
    git -C "${TMPDIR}/repo" sparse-checkout set assets 2>/dev/null
    if [ -d "${TMPDIR}/repo/assets" ]; then
        mkdir -p "$ASSETS_DIR"
        cp -r "${TMPDIR}/repo/assets/." "$ASSETS_DIR/"
        ok "installed assets to ${ASSETS_DIR}"
    else
        info "warning: could not fetch assets — whetstone setup will look for them locally"
    fi
fi

if ! echo "$PATH" | tr ':' '\n' | grep -qx "$BIN_DIR"; then
    info "note: ${BIN_DIR} is not in PATH — add it to your shell profile"
fi

ok "running whetstone setup"
exec "${BIN_DIR}/whetstone" setup "$@"
