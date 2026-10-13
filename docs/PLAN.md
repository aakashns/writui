# writui — plan

Milestones in build order. Each milestone lands on `main` through one or more
pull requests (split whenever the scope gets big), and every merge that
changes the app is released automatically and becomes what the creator
writes with — so each PR must leave the app usable. Order can change; this
file is the source of truth for what's next.

> Let's plan to do the open source and site right after M2, I want to get it
> out as soon as it's doing something. And let's bring in LLM chat sidebar
> right after the open source release.

## Next up

Alongside development, focus promotion on regular users and useful feedback.
The illustrated "Try a five-minute writing session" guide and GitHub
feedback form landed in PR #24. The guide is linked from the demo caption
and README and uses the reader's regular vault. Next, sharpen the homepage
around the writing experience and actively recruit people to try a session.
Prepare suitable community placements together; do not wait two weeks for
a directory listing before doing more outreach. Learn where new writers
get stuck and whether they return on another day.
Keep promotion work in separate PRs from the app milestones below.

The site is on Cloudflare Workers: writui.com serves it (and
`install.sh`), and www redirects. Still to do by hand: turn off GitHub
Pages for the repo (it's still enabled, with the writui.com domain).

The creator's batch of list / editor ideas shipped in `0.12.0`, and the
"Sort" dropdown and list tweaks in `0.13.0`, a Nerd Font setting,
`writui info` and Select All in `0.14.0`, Tab cycling the sort and
Ctrl/Cmd+arrow jumps in `0.15.0`, and in `0.16.0` the logo as a big banner
on the password screens, boxed password fields, Esc to clear or quit, and
the whole repo run through `cargo fmt` (checked in CI). `0.17.0` narrows
the logo, adds an Unlock button, trims the setup screen, and makes Esc on
the list quit without asking. Next:

1. **Try online** (writui.com/try), below: the creator wants it next, and
   it must work on a phone. Probably two or three PRs, in the order listed.
2. M6 (LLM chat sidebar) and on in order. The Settings screen exists now;
   add the rest of its settings as they come up.

## M0 — Writing core

The smallest thing that can replace another writing app.

- [x] Project scaffold: Cargo crate, dev data dir kept apart from real data
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
- [x] Undo / redo
- [x] Autosave (debounced + on quit)
- [x] Hint bar with the shortcuts for the current screen
- [x] Zen editor: hints only while Ctrl is held
- [x] Command menu (Ctrl+K) instead of a row of hints in the editor; the
      bar shows the word count and when the post was started
- [x] First ship: GitHub release `v0.1.0`

## M1 — Versions and history

- [x] Ctrl+S: prompt for a name, record a full snapshot (a version)
- [x] ~~"Changed since last save" indicator~~ (dropped in `0.8.0`: the post is always saved)
- [x] History view: list of versions, view full text, restore into the post
- [x] Terminology: "versions", no "drafts"; terminal title shows the post

## M5 — Selection and clipboard

Moved up: built right after M1.

Needed before the chat sidebar, so the LLM can see what's selected.
(Until then, Shift+drag, or Option+drag in iTerm2, uses the terminal's own
selection to copy.)

- [x] Mouse drag select, double-click word, Shift+movement select
- [x] Copy / cut / paste with the system clipboard
- [x] Word / paragraph movement

## M2 — Live markdown formatting

- [x] Headings, bold, italic, strikethrough, inline code, code blocks
- [x] Links, quotes, lists, horizontal rules, tables; markdown symbols faded
- [x] Links open in the browser (Ctrl+click, or the menu)
- [x] List items and quotes wrap with a hanging indent
- [x] Stays fast on long posts
- [ ] Syntax highlighting in code blocks, per language (later)

## M3 — Getting writing out + project site

- [x] Copy as markdown (whole post)
- [x] Export as markdown file
- [x] `site/` (Zola): landing page, blog, changelog
- [x] First changelog / blog post, written in writui and exported into `site/content/`
- [x] Front matter on export, remembered per post; Tab completes paths
- [x] Import a markdown file as a post (front matter kept; exports go back to it)
- [x] Import text files too; imported posts are dated by the file; exports
      start in Documents

## M4 — Open source release

The repo went public early (during M0), so most of this is already done.

- [x] License (MIT), README, contributing notes
- [x] GitHub Actions: build, test, lint
- [x] Release builds (macOS and Linux, each x86_64 and arm64), published
      automatically when a merge bumps the version
- [x] Windows: build from source (README), with how to run the tests there
- [x] Site deployed (Cloudflare Workers, at writui.com)
- [x] Make the repo public
- [x] One-line install script (`curl … | bash`)
- [x] `writui upgrade` and `writui --version`
- [x] ~~`writui upgrade` asks for the password and migrates the vault right away~~
      (dropped in `0.10.0`: the next unlock migrates, without a second password)
- [x] Migration backups deleted once the migrated vault passes integrity checks

## Try online

writui in the browser at writui.com/try, phones included (see the spec's
"Try online").

- [ ] Separate the screens from the terminal and the vault, so they can run
      on another backend (the real app behaves the same)
- [ ] Web build with Ratzilla: in-browser storage, no encryption / files /
      upgrade, other keys for shortcuts the browser keeps; CI builds it
- [ ] Phones: hidden text field for the on-screen keyboard (autocorrect,
      swipe typing), extra-keys row (Esc, Ctrl, Tab, arrows), tap = click,
      touch scrolling
- [ ] Every screen works at phone width (about 40–50 columns)
- [ ] Published at writui.com/try, tested on a real iPhone and Android
      phone, then linked from the landing page

## M6 — LLM chat sidebar

- [x] Settings screen, stored in the vault: author (for front matter), line width
- [ ] More settings: what Tab types, and the rest listed in the spec
- [ ] API keys for OpenAI + Anthropic in settings (or prompt on first open)
- [ ] Sidebar that fits next to the editor (or takes over on narrow terminals)
- [ ] Context sent automatically: post, selection, cursor position
- [ ] Streaming replies, model picker
- [ ] Conversations saved per post, resume picker

## M7 — LLM edits

- [ ] LLM proposes edits; shown as a diff in the editor
- [ ] Accept / reject per edit

## M8 — Search

- [x] Search across full post bodies (and front matter) on the list screen,
      inside the encrypted vault (no index: posts are scanned as you type)

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

## List and editor polish

The creator's batch of ideas, after M3.

- [x] "New" / "Import" with their own icons; single-key shortcuts on the
      list; Esc asks before quitting
- [x] On macOS, Cmd+letter works like Ctrl+letter in the editor; Ctrl+L
      (Alacritty's Cmd+K) opens the menu too
- [x] The menu doesn't change size while filtering
- [x] Right-click menu: cut, copy, paste
- [x] Open on the screen writui was left on
- [x] Search field highlights the first match while typing; sort by
      updated / created / title
- [x] Lock (Ctrl+O), back to the same screen
- [x] "Sort" dropdown on the search row, ascending or descending;
      "New" highlighted on opening; shortcuts on "New" / "Import"
- [x] Nerd Font setting (the search icon); `writui info`; Select All in
      the right-click menu
- [x] Tab / Shift+Tab cycle the sort; Ctrl/Cmd+Up/Down to the start / end
      of the post; Ctrl/Cmd+Left/Right on by sentence or row

## Later

- [ ] Homebrew tap (maybe)
- [ ] Auto-lock after inactivity
- [ ] Change password
- [ ] Diffs between versions
- [ ] Preview follows the cursor
- [ ] Command palette
- [ ] CLI subcommands: list posts, show a post, export a post, …
- [ ] Multiple vaults (much later): the creator asked for it "later
      later", so after everything above
