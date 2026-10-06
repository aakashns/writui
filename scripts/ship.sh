#!/usr/bin/env bash
# Release the version on main and install it as the `writui` on your PATH.
#
# Run after a PR is merged. Tags main with the version in Cargo.toml, which
# makes GitHub Actions build and publish the release (.github/workflows/
# release.yml), then downloads this machine's build from the release and
# installs it. Each PR that changes the app bumps the version in Cargo.toml.
set -euo pipefail
cd "$(dirname "$0")/.."
dest="${WRITUI_INSTALL_DIR:-$HOME/.local/bin}"

fail() { echo "ship: $*" >&2; exit 1; }

git fetch --tags --prune -q origin
[[ $(git branch --show-current) == main ]] || fail "not on main"
[[ -z $(git status --porcelain) ]] || fail "uncommitted changes"
[[ $(git rev-parse HEAD) == $(git rev-parse origin/main) ]] ||
  fail "main isn't the same as origin/main (git pull first)"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
tag="v$version"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) binary=writui-macos-arm64 ;;
  Darwin-x86_64) binary=writui-macos-x86_64 ;;
  Linux-x86_64) binary=writui-linux-x86_64 ;;
  Linux-aarch64) binary=writui-linux-arm64 ;;
  *) fail "ship.sh doesn't support $(uname -s) $(uname -m)" ;;
esac

if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  [[ $(git rev-parse "$tag^{commit}") == $(git rev-parse HEAD) ]] ||
    fail "$tag is already released from an earlier commit; bump the version in Cargo.toml"
  echo "$tag is already tagged on this commit"
else
  git tag -a "$tag" -m "writui $version"
  git push -q origin "$tag"
  echo "Pushed $tag"
fi

# Wait for the release build of this tag.
echo "Waiting for the release build (a few minutes)..."
run=""
for _ in $(seq 30); do
  run=$(gh run list --workflow release.yml --branch "$tag" --limit 1 --json databaseId --jq '.[0].databaseId // empty')
  [[ -n $run ]] && break
  sleep 2
done
[[ -n $run ]] || fail "the release build for $tag didn't start"
gh run watch "$run" --exit-status --interval 10 >/dev/null ||
  fail "the release build failed: $(gh run view "$run" --json url --jq .url)"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
gh release download "$tag" --dir "$tmp" --pattern "$binary" --pattern SHA256SUMS
(cd "$tmp" && grep " $binary\$" SHA256SUMS | shasum -a 256 -c - >/dev/null) || fail "checksum mismatch"
mkdir -p "$dest"
install -m 755 "$tmp/$binary" "$dest/writui"
echo "Installed $("$dest/writui" --version) to $dest/writui"
echo "Release: $(gh release view "$tag" --json url --jq .url)"
