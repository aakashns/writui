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
overwritten. Upgrade doesn't touch the vault itself (it would need the password);
migrations run on the next unlock, as before. (Since 0.5.0 it then runs the
new binary as a hidden `writui migrate`, which asks for the password with
`rpassword` and opens the vault; skipping is always safe.) Subcommands use clap's
`Subcommand`, ready for the planned `list` / `show` / `export`.

## Migration backups are deleted after a verified migration

Before migrating a vault that holds data, it's still copied to
`writui.db.v<old version>-<time>.bak`. After the migrations, SQLCipher's
`cipher_integrity_check` (every page decrypts and is untampered) and SQLite's
`integrity_check` must both pass before the backup is deleted. If a
migration or a check fails, the backup is kept and the error names it.
Brand new vaults aren't backed up or checked.

## Autosave: a timer in the event loop, debounced

The event loop used to block until the next key or mouse event. Now, while
the open post has unstored edits, it waits only until the draft is due
(`event::poll` with a timeout), then stores it. Due means 1 second after the
last edit, or 5 seconds after the first unstored one if typing doesn't
pause, so a crash or a closed terminal window loses at most a few seconds.
Each store writes the body (only if it differs from what's stored, so
undoing back to it doesn't bump "last updated") and the cursor. If storing
fails, the editor shows the error and tries again 5 seconds later; leaving
the editor still refuses to leave unsaved writing, as before.

## Undo: snapshots of the rope, grouped by word

Each undo step keeps the whole buffer (rope + cursor) from before the edit.
`ropey` ropes share unchanged nodes between clones, so a snapshot costs
roughly the size of the change rather than the size of the post, and
restoring is exact, with no inverse operations to get wrong. Typing groups
into one step per word plus its trailing whitespace; runs of Backspace or
Delete group too; Enter, Tab, paste and any cursor move between edits start
new steps. 1000 steps are kept. History is per editor session: leaving the
post drops it (saves in M1 cover going back further).

Redo is Ctrl+Y. Ctrl+Shift+Z also works where the terminal reports Shift
with Ctrl+letter; most don't without the kitty keyboard protocol, which we
don't enable, so it's not what the hint bar shows.

## Saves: a `saves` table of full snapshots

Migration 4 adds `saves (id, post_id, name, body, created_at)`, indexed by
post and time. Each save is the post's full text, not a diff: posts are
small, full text is what the history view shows, and diffs can be computed
later from neighbouring saves. Saving also stores the draft. "Changed since
last save" compares the editor's text with the newest save's text (loaded
when the post opens), so it's exact, including after undoing back to it.
Deleting a post forever (or the 30-day purge) deletes its saves in the same
transaction; SQLite's foreign keys stay off, so this is done explicitly.

## History opens over the editor; Ctrl+R

The history view lives inside the editor screen rather than being a screen
of its own, so the open post, its undo history and autosave all carry on
underneath. Restoring replaces the text as one undo step. The shortcut is
Ctrl+R: Ctrl+H is Backspace in many terminals, and Ctrl+R matches the
shell's history search.

## Selection and the clipboard

The selection is an "anchor" kept next to the cursor in the editor's text
buffer: the selection runs between them, so moving the cursor with Shift
held extends it and every edit simply clears it. Edits that replace a
selection are always a separate undo step. The system clipboard is reached
with the `arboard` crate (no image support, to keep it small); it's opened
on first use and kept for the whole run, because on Linux the program that
copied serves the clipboard. OSC 52 (terminal-side clipboard) was passed
over: it can't be read back, so it couldn't do Ctrl+V. Selection is drawn
with the reverse-video attribute, so it works in any terminal colour scheme.
Terminals disagree about word-movement keys (Option+Arrow arrives as Alt+B/F
in some), so several are accepted.

## Hints show while Ctrl is held: the kitty keyboard protocol

Terminals normally send nothing when Ctrl alone is pressed. The kitty
keyboard protocol can report every key, modifiers included, with press and
release events, so writui turns it on (when the terminal answers the query
for it) with "report all keys", "event types" and "alternate keys". That
changes how all keys arrive: held keys come as repeats (treated as presses),
releases are ignored, Shift+letter arrives as the shifted character
(Ctrl+Shift+Z arrives as Ctrl+"Z", so that's redo too), and Caps Lock no
longer capitalises on its own, so writui does it. Every key and mouse event
carries the held modifiers, so a missed Ctrl release corrects itself on the
next event. Where the protocol isn't available, hints always show.

Risk: in this mode, characters typed with Option on macOS (em dash, é)
arrive as Option+key; crossterm can't read the "associated text" that would
carry the composed character. To check per terminal; if it's lost, the fix
is to parse that text ourselves or drop "report all keys".

## Live formatting: a CommonMark parser, text styles, faded symbols

The editor parses the whole post with `pulldown-cmark` (CommonMark, plus
GitHub's strikethrough, task lists and tables) each time the text changes,
and styles the source from the parser's byte ranges. A real parser means
text is formatted only when it will really render that way, which is the
point ("so I know I'm getting the intended effect"); a half-typed `**bold`
stays plain. Whatever the parser skips over between the pieces of text it
reports is markdown syntax (`#`, `**`, `>`, bullets, a link's `](url)`), so
it's faded, with no per-construct rules.

Styles are text attributes only (bold, italic, underline, strikethrough,
faint), never colours, except code in green: colour pulls the eye and breaks
the flow of writing. Attributes look right in any colour scheme. Faint
(SGR 2) works in Alacritty, Ghostty and iTerm2.

Re-parsing everything is simple and fast enough: about 20 ms for a 210 KB
post in a release build (a long book chapter), and well under a millisecond
for a normal post. If very long posts ever feel slow, the fix is parsing
only from the block before the edit.

## Opening links: Ctrl+O and Ctrl+click, web and email only

writui handles the mouse itself, so a plain click on a link must still place
the cursor. Links open with Ctrl+O (shown in the hint bar only while the
cursor is on a link, so it's clickable there too) or Ctrl+click. They open
with the system's handler (`open`, `xdg-open`), which would also run files
and apps, so only `http://`, `https://` and `mailto:` links open. OSC 8
terminal hyperlinks (Cmd+click handled by the terminal) were passed over:
ratatui draws cell by cell, and the escape codes would have to be smuggled
through it. Web addresses written out in the text count as links too.

## Hanging indents are layout only

Wrapped rows of a list item or quote are indented to line up under its text.
The indent is part of the soft-wrap layout (each row knows its indent), not
spaces in the text, so the markdown saved is exactly what was typed. It's
found from the line's own prefix (`- `, `1. `, `> `, `- [ ] `, after any
indentation), not the parser, and never takes more than half the width.

## The editor uses the whole screen

The blank row above the title is part of the scrolling text, not a fixed
margin: at the top of a post it pads the title, further down it scrolls away
and the text starts on the first row. In zen (where the terminal reports
Ctrl on its own), the text also runs to the last row; holding Ctrl draws the
saved state and hints over the bottom two rows instead of resizing the text,
so nothing jumps. A passing message covers only its own row. Where hints
always show, their two rows stay reserved, so they never hide the text.

## Windows leaves CI

Windows was the slowest CI job (about twice as long as the others), for a
platform with no release binaries. CI now runs on Linux
and macOS only. The README's Windows section says how to run the tests and
clippy there by hand.

## Saves become versions; the table keeps its name

What Ctrl+S makes is now called a version, and the continuously saved post
is just "the post" (no more "draft"). The code says `Version` throughout,
but the database table is still `saves`: renaming it would be a migration
(with a backup) for no change in behaviour. Without the saved-state
indicator, the zen editor's status row is drawn over the text only while a
message is showing; holding Ctrl covers just the bottom row, with the hints.

## Line ends on Ctrl+Left/Right, and Cmd on macOS

Ctrl+Left/Right go to the start / end of the row (like Home / End), which
moves word jumps to Alt/Option+Left/Right only. Cmd+Left/Right do the same,
but only in macOS builds: elsewhere the Super key is the Windows key, which
the system usually keeps for itself. Cmd arrives only where the terminal
passes it on (Alacritty with the kitty keyboard protocol does; iTerm2 and
Ghostty by default turn it into something else or keep it).

## The terminal title

The window title is set with the standard OSC title sequence: the post's
title in the editor, "writui" elsewhere. It's only sent when it changes.
The terminal's own title is pushed onto xterm's title stack at start and
popped on the way out (also after a panic); terminals without a title stack
ignore both, and shells usually set their own title at the next prompt
anyway. Control characters are dropped from post titles before they're
sent.

## A command menu instead of a row of hints

The editor's hint bar had grown past one row on a zoomed-in terminal. Now
it only says "Ctrl+K menu", and Ctrl+K opens a filterable list of every
editor command with its shortcut. Ctrl+K rather than the Ctrl+Space first
asked for: on macOS, Ctrl+Space switches keyboard input sources whenever
there's more than one, and writui would never see it; Cmd/Ctrl+K is also
the usual "command palette" key (VS Code, Slack, Linear). The menu is its
own widget (`tui/menu.rs`), generic over the screen's commands like the
dialogs, so other screens can get one later. The word count skips runs
without a letter or digit, so markdown's `#`, `-` and `---` don't count;
it's counted only when the bar needs it after an edit.

## Copy / export as markdown on Ctrl+Shift+C / Ctrl+Shift+S

Copy as markdown is "copy, but everything" (Ctrl+Shift+C); export is "Save
as" (Ctrl+Shift+S, Ctrl+S being "save version"). Both need a terminal that
tells Shift apart with Ctrl (the kitty keyboard protocol: Alacritty, Ghostty,
kitty, WezTerm); elsewhere they're Ctrl+C / Ctrl+S, and the menu always has
them. Ctrl+E was avoided: Ghostty turns Cmd+Right into Ctrl+E. With the
kitty protocol, Ctrl+Shift+C arrives as Ctrl+"C"; so that Caps Lock can't
turn Ctrl+C into it, Caps Lock no longer capitalises letters typed with
Ctrl (which also stops Caps Lock + Ctrl+Z redoing). Exports are written as
plain files, outside the vault: that's the point of them.

## `writui upgrade` leaves the vault to the next unlock

From 0.5.0, upgrade ran the new binary as `writui migrate`, which asked for
the password and migrated right away. Dropped in 0.10.0: the next unlock
migrates anyway (with the same backup and checks), so it only meant typing
the password twice. `rpassword` went with it. `migrate` stays as a hidden
command that does nothing, because 0.5.0 to 0.9.0 still run it on the new
binary after upgrading, and an unknown command would end their upgrade with
an error.

## The bar's date stays in the app's style

The editor bar shows when a post was started as `Oct 8, 2026, 10:46 AM`,
like History, rather than the mockup's `8 Oct 2026, 9:14` (creator's call,
after 0.9.0).

## The project site: Zola, GitHub Pages from Actions

`site/` builds with Zola 0.23 (Tera 2 templates), no theme, one stylesheet
and no JavaScript. Inter is served from the site itself (variable woff2,
OFL licence alongside), not from Google Fonts. The site is published with
GitHub's Pages actions (not a `gh-pages` branch). The workflow builds with
the address Pages reports, so the site works at `aakashns.github.io/writui`
until writui.com's DNS is set, and at writui.com after. There's no
changelog page: "Changelog" links to the GitHub releases, whose notes list
the merged PRs (creator's call; an automatic page built from them was
tried first). Zola refuses markdown files
without a front matter header, and writui exports plain markdown, so
`site/post.sh` adds the header (title from the `# Title` line, today's
date, kept on later updates, even from git if the export replaced the
file). The landing page's demo is an MP4 recorded with VHS (about 200 KB,
sharper and smaller than a GIF), with its last frame as the poster.

## Site logo, icons and SEO

The logo is `❯|`, a prompt and a typing cursor, drawn as two strokes in a
64×64 square: the same stroke width (6), the same height (y 18 to 46), and
round caps and joins. The pair is centred both ways. The prompt is the
text colour, the cursor the green used for code. SVG favicon with an ICO
and an apple-touch-icon as fallbacks, a web manifest with 192 / 512 /
maskable icons, and a 1200×630 social card. Every page has a canonical
URL, a description (a post's first 160 characters unless it sets its own)
and Open Graph / Twitter tags; the landing page has `SoftwareApplication`
JSON-LD. Zola writes `sitemap.xml` and `robots.txt` (which points to the
sitemap) itself. The 404 page is `noindex`.

## Install from writui.com/install.sh

The install command is `curl -fsSL https://writui.com/install.sh | bash`.
`site/static/install.sh` is a symlink to the repo's `install.sh`, which
Zola copies into the built site as a plain file, so the two can't drift;
the site workflow also runs when `install.sh` changes. The script always
fetches the latest release, so the copy on `main` and the one attached to
each release behave the same, and the release URL keeps working.

## The project site moves to Cloudflare Workers

Replaces the GitHub Pages publishing above (creator's call: Pages was slow
to get writui.com a certificate). writui.com's DNS is on Cloudflare, and
the site is a Worker with static assets only, no code. Everything for it
lives in `site/`: `wrangler.jsonc` (the Worker's name, `public/` as the
assets, Zola's `404.html` for missing pages, and writui.com as its custom
domain) and `build.sh`. Cloudflare's Git integration runs `build.sh` then
`npx wrangler deploy`, with `site` as the root directory, on every push to
`main`, and gives other branches preview links; there's no site workflow
in GitHub Actions any more. Cloudflare's Workers build image has no Zola,
so `build.sh` downloads the latest release (creator's call: latest, not
pinned); a Zola release with breaking changes fails the build and leaves
the last deploy live. `install.sh` at the repo root is still served from
the symlink in `site/static/`.
