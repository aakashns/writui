# writui

Terminal-based, encrypted markdown writing app in Rust (ratatui). Built mainly
for Aakash's own daily writing, developed as if it's open source.

- Product spec: `docs/SPEC.md` — quoted parts are Aakash's own words; keep
  them verbatim. Update the **Details** when decisions change.
- Plan: `docs/PLAN.md` — milestones with checkboxes. Tick items as they land.
- Technical decisions: `docs/DECISIONS.md` — append, don't rewrite.

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
2. Verify by running the app in tmux and reading the screen
   (`tmux capture-pane`), plus `cargo test` and `cargo clippy`.
3. Open the PR with `gh pr create`. The description says what changed in
   behavioural terms and how to try it.
4. Aakash tries it and merges. Never merge PRs yourself.
5. After a merge: pull `main` and ship with `scripts/ship.sh` (release build,
   install to `~/.local/bin/writui`). Tick the items in `docs/PLAN.md`.
