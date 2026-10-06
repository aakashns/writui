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
     shipping hasn't started yet. It tags the version, waits for GitHub
     Actions to publish the release, and installs that build.
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
.github/      CI and release workflows
```

## Rules

- **Aakash's real writing is sacred.** Debug builds use `./.dev-data/`, never
  the real vault. Dev and throwaway vaults always use the shared password
  **`writui-dev`** (debug builds show it on the unlock screen). Every schema
  change is a migration; never drop or rewrite user data destructively. Back
  up the vault before migrating.
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
   real app in tmux against a throwaway vault:
   - `tmux new-session -d -s w -x 100 -y 30 "target/debug/writui --db /tmp/x/writui.db"`,
     wait ~1.5s for it to start, then `tmux send-keys` and
     `tmux capture-pane -p` (add `-e` to see styles).
   - Mouse clicks can be sent as SGR sequences (1-based):
     `tmux send-keys -t w -l $'\e[<0;COL;ROWM\e[<0;COL;ROWm'`.
   - When a PR adds a migration, check it against a vault made by the
     previous build: the backup file appears and the data survives.
   - Don't build an old commit into the shared `target/` (e.g. from a
     worktree): cargo then leaves the stale binary in `target/debug/`. Use a
     separate `CARGO_TARGET_DIR`, or `touch src/main.rs` before rebuilding.
3. Open the PR with `gh pr create`. The description says what changed in
   behavioural terms, has a **Try it** section with exact commands
   (`gh pr checkout <n> && cargo run` — debug builds use `.dev-data/`, never
   the real vault), and screenshots.
   - Screenshots: write a VHS tape (`brew install vhs`) that drives the debug
     binary against a throwaway `--db`, with `Screenshot` steps (and a GIF
     `Output` for flows). VHS gotchas: one command per line, quote file
     paths, wrap the launch command in `Hide` / `Show`, and there are no
     `Home` / `End` keys (use arrows).
   - Attach images with gh's built-in `--attach` (on `gh pr create`,
     `pr edit`, `pr comment`): put the images and a body file in one folder,
     reference them as `![alt](./name.png)` in the body, and run e.g.
     `gh pr edit <n> --body-file body.md --attach ./name.png --attach ./flow.gif`
     from that folder (add `-R aakashns/writui`, since that folder isn't
     the repo). gh uploads them and rewrites the references.
4. Aakash tries it locally and merges. Never merge PRs yourself.
5. After a merge: pull `main` and ship with `scripts/ship.sh`. It tags `main`
   with the version in `Cargo.toml`, GitHub Actions builds and publishes the
   release (`.github/workflows/release.yml`), and the script installs that
   build to `~/.local/bin/writui`. Tick the items in `docs/PLAN.md`.

## Versions and releases

The repo is public, and every ship is a GitHub release with notes generated
from the merged PRs. So every PR that changes the app bumps `version` in
`Cargo.toml` (and `Cargo.lock`, via `cargo build`): minor for new behaviour,
patch for fixes only (`0.1.0` → `0.2.0` / `0.1.1`). Docs-only PRs don't bump
and don't ship. The PR title becomes a line in the release notes, so make it
read well to someone who isn't Aakash. CI (`.github/workflows/ci.yml`) runs
clippy and the tests on Linux and macOS for every PR; keep it green.
