#!/usr/bin/env bash
# Puts a post exported from writui (Ctrl+Shift+S) into the blog:
#
#   site/post.sh ~/Desktop/morning-pages.md
#
# The title comes from the post's first line (`# Title`), and the date is
# today's. It writes site/content/blog/<same file name>, with the header
# Zola needs on top. Export straight into site/content/blog/ and it fixes
# the file up in place. Running it again for a post that's already there
# (after editing and exporting it again) updates the text and keeps its
# original date, even when the export replaced the file in the blog.
set -euo pipefail

if [[ $# -ne 1 || ! -f $1 ]]; then
  echo "usage: site/post.sh <file exported from writui>" >&2
  exit 1
fi
src=$1
dest="$(cd "$(dirname "$0")" && pwd)/content/blog/$(basename "$src")"

first=$(head -n 1 "$src")
if [[ $first == "+++" || $first == "---" ]]; then
  echo "$src already has a header; nothing to do." >&2
  exit 0
fi
if [[ $first != "# "* ]]; then
  echo "$src doesn't start with a '# Title' line." >&2
  exit 1
fi
title=${first#"# "}
title=${title//\\/\\\\}
title=${title//\"/\\\"}

today=$(date +%Y-%m-%d)
date=$today
updated=
old=
if [[ -f $dest ]] && [[ $(head -n 1 "$dest") == "+++" ]]; then
  old=$(sed -n 's/^date = //p' "$dest" | head -n 1)
elif [[ -f $dest ]]; then
  # Exported over the published post: its date is in the last commit.
  old=$(cd "$(dirname "$dest")" && git show "HEAD:./$(basename "$dest")" 2>/dev/null |
    sed -n 's/^date = //p' | head -n 1) || true
fi
if [[ -n $old && $old != "$today" ]]; then
  date=$old
  updated=$today
fi

tmp=$(mktemp)
{
  echo "+++"
  echo "title = \"$title\""
  echo "date = $date"
  [[ -n $updated ]] && echo "updated = $updated"
  echo "+++"
  echo
  # The body, without the title line and the blank lines after it.
  awk 'NR > 1 && (started || NF) { started = 1; print }' "$src"
} > "$tmp"
mv "$tmp" "$dest"
echo "Added to the blog: ${dest#"$(pwd)/"}"
