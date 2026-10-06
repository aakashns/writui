# Contributing to writui

writui is built mainly for its author's own daily writing, and developed in
the open. Bug reports and ideas are very welcome as
[issues](https://github.com/aakashns/writui/issues). For anything bigger
than a small fix, please open an issue before a pull request, so we can
agree on the approach first.

## Where things are

```
src/          the app (a single Rust crate)
docs/         SPEC.md (what we're building), PLAN.md (milestones, what's
              next), DECISIONS.md (technical choices and why)
install.sh    the one-line installer for macOS and Linux
.github/      CI and release workflows
```

`CLAUDE.md` describes the day-to-day workflow in detail: how PRs are put
together, tested and shipped.

## Build and run

You need [Rust](https://rustup.rs) 1.85 or newer and a C compiler (Xcode
command line tools on macOS, `build-essential` on Debian/Ubuntu, Visual
Studio Build Tools plus Strawberry Perl on Windows, building from PowerShell).
The first build takes a few minutes, because it compiles the encryption
libraries (SQLCipher and OpenSSL) from source.

```sh
cargo run
```

**Debug builds never touch your real vault.** They keep their vault and
config in `.dev-data/` inside the repo, and that vault's password is always
`writui-dev` (the unlock screen of a debug build says so). For a throwaway
vault, pass `--db /tmp/somewhere/writui.db` and use the same password.

To install your own release build instead of a downloaded one:

```sh
cargo install --locked --path .
```

## Before opening a pull request

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

CI runs both on Linux, macOS and Windows for every pull request. Also try
the change in the real app, in a couple of different terminals if it
touches what's drawn on screen. A few rules the app sticks to:

- Only the terminal's 16 standard colours, so it fits any theme.
- Everything works with the mouse, everything also has a shortcut, and the
  shortcuts are shown in the hint bar.
- Changes to what's stored in the vault are migrations. The vault is backed
  up before a migration runs, and the backup is only removed once the
  migrated vault passes its integrity checks. Never lose someone's writing.

Pull request descriptions explain what changes for the person using the
app, with screenshots for anything visible.

## Versions and releases

Every PR that changes the app bumps `version` in `Cargo.toml`: minor for new
behaviour, patch for fixes only. Merging a PR with a new version releases it
automatically: GitHub Actions builds the macOS and Linux binaries and
publishes them, with `install.sh` and notes listing the merged PRs, as a
GitHub release. PRs that keep the version (docs only) release nothing.

## License

writui is under the [MIT license](LICENSE.txt). By contributing, you agree
that your contributions are licensed the same way.
