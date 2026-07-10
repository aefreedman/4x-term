#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    cat <<'EOF'
Usage: ./setup/bootstrap-macos.sh [--skip-brew]

Installs the basic macOS toolchain, Pi CLI, and project-local Pi packages.
EOF
}

skip_brew=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        --skip-brew) skip_brew=true ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

if [ "$(uname -s)" != "Darwin" ]; then
    echo "This bootstrap currently supports macOS only." >&2
    exit 1
fi

if [ "$skip_brew" = false ]; then
    if ! command -v brew >/dev/null 2>&1; then
        echo "Homebrew is required. Install it from https://brew.sh and rerun." >&2
        exit 1
    fi
    echo "==> Installing Homebrew dependencies"
    brew bundle --file "$SCRIPT_DIR/Brewfile"
fi

command -v npm >/dev/null 2>&1 || {
    echo "npm is unavailable. Rerun without --skip-brew or install Node.js." >&2
    exit 1
}

echo "==> Installing the latest Pi CLI"
npm install --global --ignore-scripts @earendil-works/pi-coding-agent@latest

pi_settings="$SCRIPT_DIR/../.pi/settings.json"
if [ -e "$pi_settings" ]; then
    echo "==> Keeping existing machine-local Pi settings"
else
    echo "==> Creating machine-local Pi settings"
    mkdir -p "$(dirname "$pi_settings")"
    node - "$SCRIPT_DIR/pi-packages.txt" "$pi_settings" <<'NODE'
const fs = require("fs");
const [manifest, output] = process.argv.slice(2);
const packages = fs.readFileSync(manifest, "utf8")
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(line => line && !line.startsWith("#"));
fs.writeFileSync(output, JSON.stringify({ packages }, null, 2) + "\n");
NODE
fi

echo "==> Installing or updating project Pi packages"
pi update --extensions --approve

if command -v rustup >/dev/null 2>&1; then
    echo "==> Ensuring the stable Rust toolchain is installed"
    rustup toolchain install stable
    rustup default stable
fi

echo
echo "Bootstrap finished. Run ./setup/doctor.sh to verify the environment."
