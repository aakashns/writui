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

Every [release](https://github.com/aakashns/writui/releases/latest) has a
ready-to-run binary for each platform:

| Platform              | Binary                       |
| --------------------- | ---------------------------- |
| macOS (Apple Silicon) | `writui-macos-arm64`         |
| macOS (Intel)         | `writui-macos-x86_64`        |
| Linux (x86_64)        | `writui-linux-x86_64`        |
| Linux (arm64)         | `writui-linux-arm64`         |
| Windows (x86_64)      | `writui-windows-x86_64.exe`  |
| Windows (arm64)       | `writui-windows-arm64.exe`   |

On macOS and Linux, download it into a folder on your `PATH` and make it
executable, e.g. for an Apple Silicon Mac:

```sh
curl -fLo ~/.local/bin/writui https://github.com/aakashns/writui/releases/latest/download/writui-macos-arm64
chmod +x ~/.local/bin/writui
```

`SHA256SUMS` in each release lists the checksums. The macOS binaries aren't
signed yet: downloading with `curl` as above works, but if you download with a
browser and macOS refuses to open it, run
`xattr -d com.apple.quarantine ~/.local/bin/writui`.

Windows builds are made and tested by CI, but nobody has tried them by hand
yet. Reports welcome.

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
