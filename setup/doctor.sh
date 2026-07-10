#!/usr/bin/env bash

set -u

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
failures=0

pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; failures=$((failures + 1)); }

check_command() {
    if command -v "$1" >/dev/null 2>&1; then
        pass "$1 is available ($(command -v "$1"))"
    else
        fail "$1 is unavailable"
    fi
}

echo "Project root: $ROOT_DIR"
echo

check_command git
check_command node
check_command npm
check_command pi
check_command rustc
check_command cargo
check_command op

if [ -f "$ROOT_DIR/.pi/settings.json" ]; then
    pass "Project Pi settings exist"
else
    fail "Project Pi settings are missing"
fi

if git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    pass "Project root is a Git worktree"
else
    fail "Project root is not a Git worktree"
fi

echo
if [ "$failures" -eq 0 ]; then
    echo "Environment is ready."
    exit 0
fi

echo "Environment needs attention: $failures failure(s)."
exit 1
