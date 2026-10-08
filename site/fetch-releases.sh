#!/usr/bin/env bash
# Saves writui's GitHub releases as site/data/releases.json, which the
# changelog page is built from: each version, its date, and the merged pull
# requests listed in its notes. Needs the GitHub CLI (`gh`). The site
# workflow runs it before every build; run it yourself to see the changelog
# in `zola serve`.
set -euo pipefail

out="$(cd "$(dirname "$0")" && pwd)/data/releases.json"
mkdir -p "$(dirname "$out")"
gh api 'repos/aakashns/writui/releases?per_page=100' --jq '[
  .[] | select((.draft or .prerelease) | not) | {
    version: (.tag_name | ltrimstr("v")),
    url: .html_url,
    date: .published_at[:10],
    changes: [
      .body // "" | splits("\r?\n")
      | capture("^\\* (?<title>.+) by @\\S+ in (?<url>https://\\S+)$")
    ]
  }
]' > "$out"
echo "Saved $(grep -o '"version"' "$out" | wc -l | tr -d ' ') releases to $out"
