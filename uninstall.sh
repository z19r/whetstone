#!/usr/bin/env bash
# Whetstone uninstall — removes CLI wrappers, optional global tools & project.
set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }

confirm() {
    local prompt="$1"
    if [[ "${WHETSTONE_UNINSTALL_YES:-}" == "1" ]]; then
        return 0
    fi
    read -r -p "$prompt [y/N] " reply
    [[ "${reply,,}" == "y" || "${reply,,}" == "yes" ]]
}

PROJECT_DIR="$(pwd)"
CLAUDE_DIR="$HOME/.claude"

remove_bins() {
    info "Removing whetstone wrappers from ~/.local/bin..."
    rm -f "$HOME/.local/bin/whetstone" "$HOME/.local/bin/whetstone-rtk"
    ok "Removed whetstone, whetstone-rtk (if present)"
}

remove_rtk() {
    if command -v rtk &>/dev/null && rtk gain &>/dev/null; then
        rtk init -g --uninstall 2>/dev/null || true
    fi
    rm -f "$HOME/.local/bin/rtk"
    rm -rf "$HOME/.local/share/rtk"
    ok "RTK removed (or was absent)"
}

remove_headroom() {
    if command -v uv &>/dev/null; then
        uv pip uninstall -y headroom-ai 2>/dev/null || \
            uv tool uninstall headroom-ai 2>/dev/null || true
    fi
    ok "Headroom uninstall attempted"
}

remove_project_memstack() {
    local claude="$PROJECT_DIR/.claude"
    if [[ ! -d "$claude/skills" ]]; then
        info "No .claude/skills in $PROJECT_DIR"
        return 0
    fi
    rm -rf "$claude/skills" "$claude/db" "$claude/rules" "$claude/commands"
    rm -f "$claude/MEMSTACK.md" "$claude/config.local.json"
    rm -f "$PROJECT_DIR/STACK-SETUP.md"
    ok "Project MemStack files removed"
}

strip_profile_comment() {
    # Best-effort: remove our Headroom export block (manual cleanup may remain).
    warn "Review shell rc files and remove ANTHROPIC_BASE_URL if unwanted."
}

main() {
    echo ""
    echo -e "${BLUE}Whetstone uninstall${NC}"
    echo "Project dir: $PROJECT_DIR"
    echo ""

    remove_bins

    if confirm "Remove RTK (global)?"; then
        remove_rtk
    else
        warn "Skipped RTK removal"
    fi

    if confirm "Remove Headroom package?"; then
        remove_headroom
    else
        warn "Skipped Headroom removal"
    fi

    if confirm "Remove MemStack from this project directory?"; then
        remove_project_memstack
    else
        warn "Skipped project MemStack removal"
    fi

    strip_profile_comment
    info "Restore ~/.claude/settings.json from .bak.* backups if needed."
    echo ""
    ok "Whetstone uninstall finished."
}

main "$@"
