# writui

A terminal writing app. Markdown, encrypted, no nonsense.

writui keeps all your posts in a single encrypted file (SQLCipher, with the
key derived from your password using Argon2id). It works with the mouse and
the keyboard, and uses your terminal's own colours.

**Status: early.** writui is being built in the open, milestone by milestone
([plan](docs/PLAN.md), [spec](docs/SPEC.md)). It's usable for writing, but
expect rough edges, and keep a copy of anything you can't afford to lose.
There is no password recovery: if you forget it, your writing is gone.

## Install

Download the archive for your machine from the
[latest release](https://github.com/aakashns/writui/releases/latest)
(macOS Apple Silicon / Intel, Linux x86_64 / arm64), unpack it, and put
`writui` somewhere on your `PATH`:

```sh
tar -xzf writui-*.tar.gz
mv writui-*/writui ~/.local/bin/
```

The macOS binaries aren't signed yet. If you downloaded with a browser and
macOS refuses to open it, run `xattr -d com.apple.quarantine ~/.local/bin/writui`.

Or build from source with Rust 1.85 or newer:

```sh
cargo install --locked --git https://github.com/aakashns/writui
```

## Use

```sh
writui
```

The first run asks you to choose a password and creates the vault at
`~/Library/Application Support/writui/writui.db` (macOS) or
`~/.local/share/writui/writui.db` (Linux). To keep it somewhere else, pass
`--db PATH`, or put `vault = "~/path/to/writui.db"` in
`~/.config/writui/config.toml`.

## Develop

```sh
cargo run
```

Debug builds never touch your real vault: they use `.dev-data/` in the repo,
with the password `writui-dev`. See [CLAUDE.md](CLAUDE.md) for how the project
is worked on, and [docs/DECISIONS.md](docs/DECISIONS.md) for why things are
the way they are.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in writui by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
