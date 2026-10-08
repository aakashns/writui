#!/bin/sh
# Install writui (https://github.com/aakashns/writui) on macOS or Linux:
#
#   curl -fsSL https://writui.com/install.sh | bash
#
# writui.com serves this file from the repo; every release has it attached
# too, at https://github.com/aakashns/writui/releases/latest/download/install.sh
#
# Downloads the latest release for this computer, checks it against the
# release's SHA256SUMS and installs it to ~/.local/bin/writui. Running it
# again upgrades. Settings, as environment variables:
#
#   WRITUI_VERSION      install this version instead of the latest (e.g. 0.1.0)
#   WRITUI_INSTALL_DIR  install here instead of ~/.local/bin
set -eu

repo="https://github.com/aakashns/writui"
dir="${WRITUI_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
  printf 'writui install: %s\n' "$*" >&2
  exit 1
}

case "$(uname -s)" in
  Darwin) os=macos ;;
  Linux) os=linux ;;
  MINGW* | MSYS* | CYGWIN*) fail "on Windows, build writui from source: $repo#windows" ;;
  *) fail "there's no writui build for $(uname -s); see $repo#install" ;;
esac

case "$(uname -m)" in
  arm64 | aarch64) arch=arm64 ;;
  x86_64 | amd64) arch=x86_64 ;;
  *) fail "there's no writui build for $(uname -m) processors; see $repo#install" ;;
esac

# A shell running under Rosetta reports x86_64 on Apple Silicon Macs.
if [ "$os-$arch" = macos-x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null)" = 1 ]; then
  arch=arm64
fi

case "$os-$arch" in
  macos-arm64) label="macOS (Apple Silicon)" ;;
  macos-x86_64) label="macOS (Intel)" ;;
  linux-arm64) label="Linux (arm64)" ;;
  linux-x86_64) label="Linux (x86_64)" ;;
esac

command -v curl >/dev/null || fail "curl is needed to download writui"

binary="writui-$os-$arch"
if [ -n "${WRITUI_VERSION:-}" ]; then
  url="$repo/releases/download/v${WRITUI_VERSION#v}"
else
  url="$repo/releases/latest/download"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "Downloading writui for $label..."
curl -fsSL "$url/$binary" -o "$tmp/writui" || fail "couldn't download $url/$binary"
curl -fsSL "$url/SHA256SUMS" -o "$tmp/SHA256SUMS" || fail "couldn't download $url/SHA256SUMS"

expected=$(awk -v name="$binary" '$2 == name { print $1 }' "$tmp/SHA256SUMS")
if command -v shasum >/dev/null; then
  actual=$(shasum -a 256 "$tmp/writui" | cut -d ' ' -f 1)
elif command -v sha256sum >/dev/null; then
  actual=$(sha256sum "$tmp/writui" | cut -d ' ' -f 1)
else
  fail "shasum or sha256sum is needed to check the download"
fi
[ -n "$expected" ] && [ "$expected" = "$actual" ] ||
  fail "the download doesn't match its checksum; please try again"

# Copy next to the destination, then rename over it, so an existing writui is
# replaced in one step and never left half-written.
mkdir -p "$dir" || fail "couldn't create $dir"
chmod 755 "$tmp/writui"
cp "$tmp/writui" "$dir/.writui.new" || fail "couldn't write to $dir"
mv -f "$dir/.writui.new" "$dir/writui"

echo "Installed $("$dir/writui" --version) to $dir/writui"

case ":$PATH:" in
  *":$dir:"*)
    found=$(command -v writui || true)
    if [ "$found" != "$dir/writui" ]; then
      echo "Note: running \`writui\` starts $found, which comes first on your PATH."
    fi
    echo "Run \`writui\` to start writing."
    ;;
  *)
    # Show paths in the home folder as $HOME/..., as people write them.
    case "$dir" in
      "$HOME"/*) shown="\$HOME/${dir#"$HOME"/}" ;;
      *) shown="$dir" ;;
    esac
    shell=$(basename "${SHELL:-sh}")
    # The ~ is printed for the user's shell to expand, not expanded here.
    # shellcheck disable=SC2088
    case "$shell" in
      zsh) rc="~/.zshrc" ;;
      bash) if [ "$os" = macos ]; then rc="~/.bash_profile"; else rc="~/.bashrc"; fi ;;
      *) rc="" ;;
    esac
    echo
    echo "$dir isn't on your PATH yet. To add it, run:"
    echo
    if [ "$shell" = fish ]; then
      echo "  fish_add_path $dir"
    elif [ -n "$rc" ]; then
      echo "  echo 'export PATH=\"$shown:\$PATH\"' >> $rc"
    else
      echo "  export PATH=\"$shown:\$PATH\"   (and add it to your shell's startup file)"
    fi
    echo
    echo "Then open a new terminal and run \`writui\`."
    ;;
esac
