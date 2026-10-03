# writui — plan

Milestones in build order. Each milestone lands on `main` through one or more
pull requests (split whenever the scope gets big), and every merge gets
installed as the binary Aakash writes with — so each PR must leave the app
usable. Order can change; this file is the source of truth for what's next.

> Let's plan to do the open source and site right after M2, I want to get it
> out as soon as it's doing something. And let's bring in LLM chat sidebar
> right after the open source release.

## M0 — Writing core

The smallest thing that can replace another writing app.

- [ ] Project scaffold: Cargo crate, `scripts/ship.sh`, dev data dir kept apart from real data
- [ ] Config file (vault location) + `--db` flag
- [ ] Vault: create on first run (password twice, no-recovery warning), unlock on later runs
- [ ] Schema migrations from day one, with a backup of the vault before migrating
- [ ] List screen: posts sorted by last updated, "New post", open with click or Enter
- [ ] Delete post (with confirmation)
- [ ] Posts: first line fixed as `# ` title; created / updated times tracked
- [ ] Editor: centred 68-char column, soft wrap, cursor movement, scrolling
- [ ] Editor: mouse click to place cursor, scroll wheel
- [ ] Undo / redo
- [ ] Draft autosave (debounced + on quit)
- [ ] Hint bar with the shortcuts for the current screen
- [ ] First ship: installed to `~/.local/bin/writui`

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

- [ ] License (MIT OR Apache-2.0), README, contributing notes
- [ ] GitHub Actions: build, test, lint
- [ ] Release builds (macOS arm64 / x86_64, Linux) on tags
- [ ] Site deployed via GitHub Pages
- [ ] Make the repo public

## M5 — Selection and clipboard

Needed before the chat sidebar, so the LLM can see what's selected.
(Until then, Shift+drag uses the terminal's own selection to copy.)

- [ ] Mouse drag select, double-click word, Shift+movement select
- [ ] Copy / cut / paste with the system clipboard
- [ ] Word / paragraph movement

## M6 — LLM chat sidebar

- [ ] Settings screen; API keys for OpenAI + Anthropic (or prompt on first open)
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
