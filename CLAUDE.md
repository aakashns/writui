# writui

Terminal-based, encrypted markdown writing app in Rust (ratatui). Built mainly
for Aakash's own daily writing, developed as if it's open source.

- Product spec: `docs/SPEC.md` — quoted parts are Aakash's own words; keep
  them verbatim. Update the **Details** when decisions change.
- Plan: `docs/PLAN.md` — milestones with checkboxes. Tick items as they land.
- Technical decisions: `docs/DECISIONS.md` — append, don't rewrite.

## When Aakash says "continue"

Each PR is usually done in a fresh conversation, so pick up the state from the
repo and GitHub, not from memory:

1. `git fetch --prune` and `gh pr list --state all --limit 5` to find the
   latest PR.
2. **Latest PR still open:** read the feedback (`gh pr view <n> --comments`,
   plus review comments via `gh api repos/aakashns/writui/pulls/<n>/comments`).
   - Feedback to address → check out that branch, fix it, push, refresh the
     screenshots if the UI changed, and reply on the PR with what changed.
   - No feedback yet → tell Aakash it's waiting on him to try and merge it,
     with the Try it commands. Don't start the next PR on top of it unless
     he asks.
3. **Latest PR merged:** `git checkout main && git pull`, then:
   - Ship (`scripts/ship.sh`) unless "Next up" in `docs/PLAN.md` says
     shipping hasn't started yet.
   - Tick what landed in `docs/PLAN.md` and update "Next up". Commit that as
     part of the next PR (never push to `main` directly).
   - If the merged PR asked questions that weren't answered in its comments,
     ask them now before they affect the next PR.
   - Start the next PR from "Next up".
4. Before ending a session, make sure the repo tells the next session
   everything: "Next up" in `docs/PLAN.md` is current, open questions are in
   the PR description, and anything decided in conversation is in
   `docs/SPEC.md` / `docs/DECISIONS.md` / this file.

## Repo layout

```
src/          Rust app (single crate)
presets/      built-in preview CSS, embedded into the binary
site/         Zola project site: landing page, blog, changelog
docs/         spec, plan, decisions
scripts/      ship.sh and other dev scripts
```

## Rules

- **Aakash's real writing is sacred.** Debug builds use `./.dev-data/`, never
  the real vault. Every schema change is a migration; never drop or rewrite
  user data destructively. Back up the vault before migrating.
- Aakash doesn't know Rust. Explain changes in terms of behaviour, not code.
- Terminal agnostic: test assumptions against Alacritty, Ghostty, iTerm2. Use
  only the 16 standard terminal colours.
- Everything mouse-operable, everything also keyboard-accessible, shortcuts
  shown in the hint bar.

## Workflow

> Let's use the pull request workflow for milestones. It's okay to do
> multiple pull requests per milestone, if while working on the milestone you
> feel the scope is too big. I don't want too much to change at once, I
> don't want to lose control over what's happening.

1. Branch off `main` for each PR. Keep PRs small and reviewable; split a
   milestone into several PRs whenever it grows.
2. Verify with `cargo test`, `cargo clippy --all-targets`, and by driving the
   real app in tmux against a throwaway vault
   (`tmux new-session -d -s w -x 100 -y 30 "target/debug/writui --db /tmp/x/writui.db"`,
   `tmux send-keys`, `tmux capture-pane -p`; add `-e` to see styles). Mouse
   clicks can be sent as SGR sequences:
   `tmux send-keys -t w -l $'\e[<0;COL;ROWM\e[<0;COL;ROWm'` (1-based).
3. Open the PR with `gh pr create`. The description says what changed in
   behavioural terms, has a **Try it** section with exact commands
   (`gh pr checkout <n> && cargo run` — debug builds use `.dev-data/`, never
   the real vault), and screenshots.
   - Screenshots: write a VHS tape (`brew install vhs`) that drives the debug
     binary against a throwaway `--db`, with `Screenshot` steps (and a GIF
     `Output` for flows). VHS gotchas: one command per line, quote file
     paths, wrap the launch command in `Hide` / `Show`. Push the images to the orphan `pr-assets` branch
     under `<branch-name>/`, and embed them with
     `https://github.com/aakashns/writui/blob/pr-assets/<branch-name>/<file>?raw=true`.
     Never merge `pr-assets` into `main`.
4. Aakash tries it locally and merges. Never merge PRs yourself.
5. After a merge: pull `main` and ship with `scripts/ship.sh` (release build,
   install to `~/.local/bin/writui`). Tick the items in `docs/PLAN.md`.
