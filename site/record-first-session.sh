#!/usr/bin/env bash
# Run from the repo root. Requires VHS and cwebp (from libwebp).
set -euo pipefail

cargo build
mkdir -p /tmp/writui-first-session-captures site/static/blog/first-session
vhs site/first-session.tape

# Crop to the relevant controls, keeping screenshot text pixel-perfect.
# In particular, exclude the setup screen's development path and note.
# -z 9 uses the strongest lossless preset; no PNGs or GIF ship on the site.
encode() {
  local screen="$1" x="$2" y="$3" width="$4" height="$5"
  cwebp -quiet -z 9 -exact -metadata none -crop "$x" "$y" "$width" "$height" \
    "/tmp/writui-first-session-captures/$screen.png" \
    -o "site/static/blog/first-session/$screen.webp"
}

encode create-vault 100 10 1000 360
encode new-post 100 10 1000 300
encode writing 100 10 1000 350
encode menu 100 10 1000 600
encode save-version 200 240 800 260
encode posts 100 10 1000 280
