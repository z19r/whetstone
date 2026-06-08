#!/usr/bin/env bash
# Whetstone installer — curl -fsSL URL | bash
# Downloads prebuilt binary from GitHub Releases, installs to ~/.local/bin,
# fetches assets to ~/.whetstone/assets/, ensures `uv` is available, then runs
# `whetstone setup` against the controlling tty so the wizard works.
set -euo pipefail

REPO="z19r/whetstone"
BIN_DIR="${HOME}/.local/bin"
ASSETS_DIR="${HOME}/.whetstone/assets"

# Make sure ~/.local/bin is on PATH for the rest of this script so newly
# installed binaries (uv, whetstone) are discoverable by the exec at the end.
case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;
    *) export PATH="${BIN_DIR}:${PATH}" ;;
esac

info()  { printf '\033[0;34m[whetstone]\033[0m %s\n' "$1"; }
ok()    { printf '\033[0;32m[whetstone]\033[0m %s\n' "$1"; }
warn()  { printf '\033[0;33m[whetstone]\033[0m %s\n' "$1" >&2; }
fail()  { printf '\033[0;31m[whetstone]\033[0m %s\n' "$1" >&2; exit 1; }

has_tty() {
    [ -r /dev/tty ] && [ -w /dev/tty ]
}

# Prompt with a default; reads from /dev/tty so it works under `curl | bash`.
# Returns 0 for yes, 1 for no. Falls back to the default in non-interactive mode.
prompt_yn() {
    local question="$1" default="${2:-Y}" reply hint
    case "$default" in
        Y|y) hint="[Y/n]" ;;
        *)   hint="[y/N]" ;;
    esac
    if ! has_tty; then
        case "$default" in
            Y|y) return 0 ;;
            *)   return 1 ;;
        esac
    fi
    printf '\033[0;34m[whetstone]\033[0m %s %s ' "$question" "$hint" >/dev/tty
    read -r reply </dev/tty || reply=""
    reply="${reply:-$default}"
    case "$reply" in
        [yY]*) return 0 ;;
        *)     return 1 ;;
    esac
}

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

install_uv() {
    info "installing uv from https://astral.sh/uv/install.sh"
    curl -LsSf https://astral.sh/uv/install.sh | sh
    # uv's installer drops the binary in $HOME/.local/bin (or $XDG_BIN_HOME).
    # Re-prepend in case the installer chose a fresh location.
    case ":${PATH}:" in
        *":${HOME}/.local/bin:"*) ;;
        *) export PATH="${HOME}/.local/bin:${PATH}" ;;
    esac
    if ! command -v uv >/dev/null 2>&1; then
        fail "uv install completed but binary not found on PATH — open a new shell and re-run"
    fi
    ok "installed uv"
}

ensure_uv() {
    if command -v uv >/dev/null 2>&1; then
        ok "uv already installed"
        return 0
    fi
    info "uv is required (whetstone uses it to install Headroom)"
    if prompt_yn "install uv now?" "Y"; then
        install_uv
    else
        fail "uv is required — install it from https://docs.astral.sh/uv/ and re-run"
    fi
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
        warn "could not fetch assets — whetstone setup will look for them locally"
    fi
fi

ensure_uv

if ! echo "$PATH" | tr ':' '\n' | grep -qx "$BIN_DIR"; then
    warn "${BIN_DIR} is not in your shell profile's PATH — add it so future shells find whetstone"
fi

# When invoked via `curl ... | bash`, stdin is the exhausted pipe and
# whetstone setup falls back to the non-interactive path (silent wizard
# bypass). Re-exec against /dev/tty so the TUI wizard actually runs.
if has_tty; then
    ok "running whetstone setup"
    exec "${BIN_DIR}/whetstone" setup "$@" </dev/tty
else
    ok "whetstone binary + assets installed"
    info "no controlling terminal detected — run \`whetstone setup\` in your shell to configure"
fi
