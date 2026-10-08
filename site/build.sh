#!/usr/bin/env bash
# Builds the site into site/public. Cloudflare runs this before deploying
# (see wrangler.jsonc); it downloads the latest Zola release when zola isn't
# installed. Locally it uses your own zola (`brew install zola`).
set -euo pipefail
cd "$(dirname "$0")"

zola=zola
if ! command -v zola >/dev/null; then
  latest=$(curl -fsSLI -o /dev/null -w '%{url_effective}' https://github.com/getzola/zola/releases/latest)
  version=$(basename "$latest")
  dir=$(mktemp -d)
  curl -fsSL "https://github.com/getzola/zola/releases/download/$version/zola-$version-$(uname -m)-unknown-linux-gnu.tar.gz" \
    | tar xz -C "$dir" zola
  zola="$dir/zola"
fi
"$zola" --version
"$zola" build
