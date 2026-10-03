#!/usr/bin/env bash
# Build a release binary and install it as the `writui` on your PATH.
set -euo pipefail
cd "$(dirname "$0")/.."
dest="${WRITUI_INSTALL_DIR:-$HOME/.local/bin}"
cargo build --release --locked
mkdir -p "$dest"
install -m 755 target/release/writui "$dest/writui"
echo "Installed $("$dest/writui" --version) to $dest/writui"
