#!/usr/bin/env bash
# Back-compat wrapper — use setup-whetstone.sh or install.sh.
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/setup-whetstone.sh" "$@"
