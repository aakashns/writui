# Technical decisions

Short log of the technical choices behind writui and why. Add new entries at
the bottom; if a decision is reversed, say so in a new entry rather than
editing history.

## Rust + ratatui + crossterm

Single small, fast binary. ratatui is the standard Rust TUI library;
crossterm is the cross-terminal backend (mouse, bracketed paste, raw mode)
and keeps us terminal agnostic.

## Our own editor component, built on `ropey`

Existing ratatui editor widgets don't handle fixed-width soft wrap, inline
markdown styling and mouse selection together. The editor is the heart of the
app, so we own it. `ropey` holds the text.

## SQLite with SQLCipher, key derived with Argon2id

SQLCipher encrypts the entire database file — titles, save names, chat
history, API keys, indexes — not just post bodies. The key is derived from the
password with Argon2id (slow to brute-force) and handed to SQLCipher as a raw
key. One encrypted file is also trivially easy to back up.

The 16-byte Argon2 salt is passed to SQLCipher along with the key, and
SQLCipher writes it as the file's first 16 bytes, so opening a vault reads
the salt from there — no sidecar file. The Argon2 cost (64 MiB, 3 passes) is
fixed in code: if it ever changes, keep the old values as a fallback so
existing vaults still open.

## Full-text search with FTS5 inside the vault

The search index lives in the same SQLCipher database, so it's encrypted too.

## Full snapshots for saves

Each save stores the whole post. Writing is small; snapshots keep history
simple and robust. Diffs can be computed on demand later.

## Config file outside the vault

The vault location must be known before unlocking, so it lives in a small
plain config file (`~/.config/writui/config.toml`), overridable with `--db`.
Nothing sensitive goes in it.

## Dev builds never touch real writing

Debug builds default to a vault in `./.dev-data/` (gitignored). Only release
builds use the real default location. Schema changes always go through
migrations, and the vault is backed up before any migration runs.

## Preview presets stored in the vault; built-ins bundled in the binary

Custom presets are CSS rows in the vault, keeping everything in one file.
Built-in presets ship inside the binary so they improve with updates; to
customise one, duplicate it into the vault.

## Local preview server

Small HTTP server (axum) bound to 127.0.0.1 with a random per-session token
in the URL. Markdown rendered with `comrak` (GitHub-flavoured).

## Datastar for the live preview

The creator wants to try [Datastar](https://data-star.dev). It fits well: the
server pushes freshly rendered HTML over server-sent events on every autosave
and Datastar patches it into the page — no hand-written client JS. Use the
official `datastar` Rust crate's axum integration. Serve `datastar.js` from
the binary rather than a CDN, so the preview works offline and the page
doesn't load third-party scripts.

## Rich text clipboard via `arboard`

Writes HTML + plain text to the system clipboard so pastes into Substack,
Gmail etc. keep formatting. (OSC 52 terminal clipboard only does plain text.)

## Zola for the project site

Static site generator distributed as a single binary, markdown-native, no
node_modules. Fits the "light and fast" spirit; deploys to GitHub Pages.

## License: MIT

Permissive and simple: one short license in `LICENSE.txt`.

## Vault logic kept separate from the TUI

Everything about the vault (unlocking, posts, drafts, saves, export) lives in
its own module with no TUI dependencies, so the planned CLI subcommands can
reuse it directly. `clap` for argument parsing from M0, since `--db` already
needs it; subcommands slot in later.

## Editor internals: ropey 1.x, graphemes, our own soft wrap

`ropey` 1.6 (2.0 is still in beta), with only `\n` counted as a line break.
Pasted text has `\r\n` / `\r` turned into `\n` and other control characters
(except tabs) dropped, so the stored markdown stays plain.

The cursor moves by grapheme (`unicode-segmentation`), so an accented letter
or an emoji is one step and never gets split. Widths come from
`unicode-width`, so CJK text wraps correctly.

Soft wrap breaks after spaces; a word longer than the column is cut. One
space may hang a cell past the column, so rows never start with the space
that ended the previous word. The whole layout is rebuilt after each edit —
fast enough for posts of any realistic size; caching per paragraph can come
with M2's "stays fast on long posts" if it's ever needed.

The editor asks the terminal for a blinking bar cursor (DECSCUSR, supported
by Alacritty, Ghostty and iTerm2), and restores the user's own cursor
everywhere else and on exit.

## Remembering the cursor: a column on `posts`

Each post stores where the cursor was when it was last closed (migration 3,
`posts.cursor`), as a character index into the body. NULL means the end, so
existing posts open as they did before. It's written whenever the editor is
left (Esc or quit), separately from the draft, and doesn't change the post's
"last updated" time. On open it's clamped to the text and moved to a
grapheme boundary, so it's safe even if the body changed some other way
(e.g. restoring an old save in M1). Failing to store it never keeps you in
the editor; failing to store the draft does.

Only the cursor is stored, not the scroll position: the view is rebuilt
around the cursor (as near mid-screen as the text allows), which works at
any terminal width.

## Releases: hand-written GitHub Actions workflow on version tags

Pushing a `vX.Y.Z` tag builds macOS (arm64, and x86_64 cross-compiled on the
same arm64 runner), Linux (x86_64, arm64, on Ubuntu 22.04 so the binaries
work with older glibc) and Windows (x86_64, arm64), and publishes bare
binaries plus a `SHA256SUMS` file. No archives: there's nothing to bundle
but the binary, and unversioned names like `writui-macos-arm64` give stable
`releases/latest/download/…` URLs.
Release notes are GitHub's generated list of merged PRs. `scripts/ship.sh`
pushes the tag and waits for the release to be published. It doesn't install
anything: the creator upgrades by downloading the release like any user. Chose a ~80-line workflow over
`cargo-dist` to keep it small and readable; we can switch if we want its
installers and Homebrew support later. macOS binaries are unsigned for now:
downloads via `curl`, `gh` or Homebrew aren't quarantined, browser downloads
need `xattr -d com.apple.quarantine`.

CI runs clippy (warnings are errors) and the tests on Linux, macOS and
Windows. No `cargo fmt --check` yet: the code isn't rustfmt-formatted, and
reformatting everything would bury real changes in a PR diff.

## Releases happen on merge; Windows is build-from-source

Replaces the tag-driven flow and `scripts/ship.sh`. The release workflow runs
on every push to `main`: if the version in `Cargo.toml` has no `v<version>`
tag yet, it builds and publishes the release (`gh release create --target`
creates the tag). Otherwise it does nothing, so docs-only merges don't
release. A tag pushed by the workflow's own token wouldn't trigger other
workflows, which is why the release is created in the same run.

Windows binaries are dropped from releases: nobody uses them by hand, and an
untried download is worse than honest build-from-source instructions. CI
still builds and tests on Windows, so building from source keeps working.

## One-line install script, attached to each release

`install.sh` (POSIX sh) picks the binary for the OS and CPU (including Apple
Silicon Macs running a Rosetta shell), checks it against `SHA256SUMS`, and
renames it into `~/.local/bin` (or `WRITUI_INSTALL_DIR`) so an existing
writui is replaced in one step. It never edits shell startup files; it prints
the line to add instead. It's attached to every release, so
`releases/latest/download/install.sh` always serves the script that matches
the latest binaries.

## macOS binaries stay unsigned

No paid Apple Developer account. The install script, `curl` and
`writui upgrade` don't set macOS's quarantine flag, so Gatekeeper doesn't
interfere; only browser downloads need `xattr -d com.apple.quarantine`.

## `writui upgrade`: ureq + sha2, replace by rename

`ureq` (blocking, small, rustls with `ring`, no async runtime) to ask the
GitHub API for the latest release and download from it; `sha2` to check the
download against `SHA256SUMS`. The new binary is written next to the running
one and renamed over it, which is atomic and fine on macOS and Linux even
while it runs. Debug builds refuse to upgrade, so `target/debug` is never
overwritten. Upgrade doesn't touch the vault (it would need the password);
migrations run on the next unlock, as before. Subcommands use clap's
`Subcommand`, ready for the planned `list` / `show` / `export`.

## Migration backups are deleted after a verified migration

Before migrating a vault that holds data, it's still copied to
`writui.db.v<old version>-<time>.bak`. After the migrations, SQLCipher's
`cipher_integrity_check` (every page decrypts and is untampered) and SQLite's
`integrity_check` must both pass before the backup is deleted. If a
migration or a check fails, the backup is kept and the error names it.
Brand new vaults aren't backed up or checked.
