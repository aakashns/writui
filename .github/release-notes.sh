#!/usr/bin/env bash
# Prints the release notes for a version: for each pull request merged since
# the last release (with a merge commit or squashed), its title, then the
# full message of each of its commits (without the Co-Authored-By lines).
#
#   .github/release-notes.sh 0.15.0 [ref]
#
# `ref` (default HEAD) is what's being released. Needs the tags and the full
# history (actions/checkout with fetch-depth: 0), and fetches squashed pull
# requests' commits from GitHub.
set -euo pipefail

# Commit messages are wrapped at about 72 columns, and GitHub shows every
# line break in release notes: join each paragraph or list item back into
# one line (leaving code blocks alone).
unwrap() {
  awk '
    function flush() { if (buf != "") print buf; buf = "" }
    /^```/ { flush(); print; fence = !fence; next }
    fence { print; next }
    /^[[:space:]]*$/ { flush(); print; next }
    /^[[:space:]]*([-*+]|[0-9]+\.)[[:space:]]/ || /^#/ { flush(); buf = $0; next }
    { if (buf == "") buf = $0; else { sub(/^[[:space:]]+/, ""); buf = buf " " $0 } }
    END { flush() }
  '
}

version=$1
ref=${2:-HEAD}
repo=https://github.com/aakashns/writui
prev=$(git describe --tags --abbrev=0 "$ref^" 2>/dev/null || true)

echo "## What's changed"
for merge in $(git rev-list --first-parent --reverse "${prev:+$prev..}$ref"); do
  subject=$(git log -1 --format=%s "$merge")
  pr=$(echo "$subject" | sed -n 's/^Merge pull request #\([0-9]*\) .*/\1/p')
  if [ -n "$pr" ]; then
    # A merge commit: GitHub puts the pull request's title in its body,
    # and its second parent is the pull request's last commit.
    title=$(git log -1 --format=%b "$merge" | head -1)
    head=$merge^2
  else
    # A squash merge: "Title (#12)". The pull request's own commits are
    # still on GitHub, at pull/12/head.
    pr=$(echo "$subject" | sed -n 's/.* (#\([0-9]*\))$/\1/p')
    [ -n "$pr" ] || continue
    title=${subject% (#"$pr")}
    head=refs/writui-notes/pr-$pr
    git fetch -q origin "+pull/$pr/head:$head"
  fi
  echo
  echo "### $title ([#$pr]($repo/pull/$pr))"
  for commit in $(git rev-list --no-merges --reverse "$merge^1..$head"); do
    echo
    echo "**$(git log -1 --format=%s "$commit")**"
    body=$(git log -1 --format=%b "$commit" | grep -vi '^co-authored-by:' || true)
    if [ -n "${body//[[:space:]]/}" ]; then
      echo
      echo "$body" | unwrap
    fi
  done
done
if [ -n "$prev" ]; then
  echo
  echo "**Full changelog:** [$prev...v$version]($repo/compare/$prev...v$version)"
fi
