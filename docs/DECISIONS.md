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

Aakash wants to try [Datastar](https://data-star.dev). It fits well: the
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

## License: MIT OR Apache-2.0

The Rust ecosystem convention. Permissive, and Apache-2.0 adds a patent grant.

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
