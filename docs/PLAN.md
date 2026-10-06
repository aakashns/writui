# writui — plan

Milestones in build order. Each milestone lands on `main` through one or more
pull requests (split whenever the scope gets big), and every merge gets
installed as the binary the creator writes with — so each PR must leave the app
usable. Order can change; this file is the source of truth for what's next.

> Let's plan to do the open source and site right after M2, I want to get it
> out as soon as it's doing something. And let's bring in LLM chat sidebar
> right after the open source release.

## Next up

M0 is split into three PRs:

1. ~~Vault create/unlock + post list~~ — PR #2
2. ~~The editor~~ — PR #3
3. **Autosave, undo/redo, editor hint bar, first ship.** Autosave needs the
   event loop to wake up on a timer (it currently blocks waiting for input).

Shipping starts after PR 3; until then, don't run `scripts/ship.sh` after
merges. PR 3 ships as the first GitHub release, `v0.1.0` (the version already
in `Cargo.toml`, so PR 3 doesn't bump it). Every app-changing PR after that
bumps the version (see CLAUDE.md).

## M0 — Writing core

The smallest thing that can replace another writing app.

- [x] Project scaffold: Cargo crate, `scripts/ship.sh`, dev data dir kept apart from real data
- [x] Config file (vault location) + `--db` flag
- [x] Vault: create on first run (password twice, no-recovery warning), unlock on later runs
- [x] Schema migrations from day one, with a backup of the vault before migrating
- [x] List screen: posts sorted by last updated, "New post", open with click or Enter
- [x] Delete post (with confirmation) → Trash (restore, delete forever, purged after 30 days)
- [x] Dialogs: arrow keys / Enter / shortcut keys / mouse
- [x] Posts: first line fixed as `# ` title; created / updated times tracked
- [x] Editor: centred 68-char column, soft wrap, cursor movement, scrolling
- [x] Editor: mouse click to place cursor, scroll wheel
- [x] Editor: reopening a post puts the cursor back where it was left
- [ ] Undo / redo
- [ ] Draft autosave (debounced + on quit)
- [ ] Hint bar with the shortcuts for the current screen
- [ ] First ship: GitHub release `v0.1.0`, installed to `~/.local/bin/writui`

## M1 — Saves and history

- [ ] Ctrl+S: prompt for a name, record a full snapshot
- [ ] "Changed since last save" indicator
- [ ] History view: list of saves, view full text, restore into draft

## M2 — Live markdown formatting

- [ ] Headings (per-level styles), bold, italic, inline code, code blocks
- [ ] Links, quotes, lists, horizontal rules
- [ ] Stays fast on long posts

## M3 — Getting writing out + project site

- [ ] Copy as markdown (whole post)
- [ ] Export as markdown file
- [ ] `site/` (Zola): landing page, blog, changelog
- [ ] First changelog / blog post, written in writui and exported into `site/content/`

## M4 — Open source release

The repo went public early (during M0), so most of this is already done.

- [x] License (MIT OR Apache-2.0), README, contributing notes
- [x] GitHub Actions: build, test, lint
- [x] Release builds (macOS, Linux, Windows; each x86_64 and arm64) on tags
- [ ] Site deployed via GitHub Pages
- [x] Make the repo public
- [ ] Signed and notarised macOS binaries (needs an Apple Developer account)
- [ ] Easier install: Homebrew tap and/or an install script

## M5 — Selection and clipboard

Needed before the chat sidebar, so the LLM can see what's selected.
(Until then, Shift+drag uses the terminal's own selection to copy.)

- [ ] Mouse drag select, double-click word, Shift+movement select
- [ ] Copy / cut / paste with the system clipboard
- [ ] Word / paragraph movement

## M6 — LLM chat sidebar

- [ ] Settings screen, stored in the vault: line width, what Tab types, and
      the rest listed in the spec
- [ ] API keys for OpenAI + Anthropic in settings (or prompt on first open)
- [ ] Sidebar that fits next to the editor (or takes over on narrow terminals)
- [ ] Context sent automatically: post, selection, cursor position
- [ ] Streaming replies, model picker
- [ ] Conversations saved per post, resume picker

## M7 — LLM edits

- [ ] LLM proposes edits; shown as a diff in the editor
- [ ] Accept / reject per edit

## M8 — Search

- [ ] Search across full post bodies on the list screen (index stays inside the encrypted vault)

## M9 — Web preview

- [ ] Local preview server (localhost only, secret token), live updates with Datastar
- [ ] Open from the editor; updates on every autosave
- [ ] Default preset: white Inter on dark

## M10 — Presets and rich text copy

- [ ] Built-in presets: Substack, Medium, Gmail
- [ ] "writui site" preset matching the project site's CSS
- [ ] Custom presets stored in the vault: import CSS file, duplicate + edit a built-in
- [ ] Switch preset from the editor
- [ ] Copy post / selection as formatted text

## Later

- [ ] Auto-lock after inactivity
- [ ] Change password
- [ ] Diffs between versions
- [ ] Preview follows the cursor
- [ ] Command palette
- [ ] CLI subcommands: list posts, show a post, export a post, …
