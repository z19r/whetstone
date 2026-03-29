# Whetstone — run tasks via `just <recipe>`

default:
    @just --list

# Full install from clone (run inside a git project root)
setup:
    bash ./setup-whetstone.sh

# Same as setup (back-compat script name)
setup-stack:
    bash ./setup-stack.sh

# Pipe-friendly installer (uses WHETSTONE_SETUP_URL if set)
install-remote:
    bash ./install.sh

# Remove whetstone wrappers and optional global/project pieces
uninstall:
    bash ./uninstall.sh

# Syntax check shell scripts
check-scripts:
    bash -n ./setup-whetstone.sh ./setup-stack.sh ./install.sh ./uninstall.sh
    bash -n ./bin/whetstone ./bin/whetstone-rtk
