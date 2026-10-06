# writui

A writing app for the terminal. Markdown, encrypted, no nonsense.

- **Private.** All your posts live in one file, encrypted with your password.
  Without the password nobody can read them.
- **Plain markdown.** You write markdown. Each post starts with a `# Title`.
- **Mouse and keyboard.** Click, scroll and press buttons with the mouse, or
  use shortcuts. The bar at the bottom of the screen always shows the
  shortcuts for where you are, and you can click those too.
- **Fits your terminal.** It uses your terminal's own colours and font, and
  works in any modern terminal (Alacritty, Ghostty, iTerm2, Windows Terminal,
  and others).

> **Early days.** writui is being built in the open, a small piece at a time
> ([what's planned](docs/PLAN.md)). It's usable for writing now, but keep a
> copy of anything you can't afford to lose. And remember: **there is no way
> to recover a forgotten password.**

## Install

Download the file for your computer from the
[latest release](https://github.com/aakashns/writui/releases/latest):

| Computer                       | File                        |
| ------------------------------ | --------------------------- |
| Mac with Apple Silicon (M1+)   | `writui-macos-arm64`        |
| Mac with Intel                 | `writui-macos-x86_64`       |
| Linux (most PCs)               | `writui-linux-x86_64`       |
| Linux (ARM, e.g. Raspberry Pi) | `writui-linux-arm64`        |
| Windows (most PCs)             | `writui-windows-x86_64.exe` |
| Windows on ARM                 | `writui-windows-arm64.exe`  |

**Mac and Linux:** paste this into your terminal, with the file name from the
table at the end of the first line:

```sh
mkdir -p ~/.local/bin
curl -fLo ~/.local/bin/writui https://github.com/aakashns/writui/releases/latest/download/writui-macos-arm64
chmod +x ~/.local/bin/writui
```

If `writui` isn't found afterwards, add `~/.local/bin` to your `PATH`. On a
Mac, if you downloaded the file with a browser instead and macOS won't open
it, run `xattr -d com.apple.quarantine ~/.local/bin/writui` once.

**Windows:** download the `.exe`, rename it to `writui.exe`, and put it in a
folder on your `PATH`. Windows support is new and hasn't had much use yet;
please [report anything odd](https://github.com/aakashns/writui/issues).

To update, download the new version the same way.

## Use

Open a terminal and run:

```sh
writui
```

The first time, writui asks you to choose a password (twice) and creates your
vault: the encrypted file that holds all your posts. After that, it asks for
the password each time it starts.

- **Posts** are listed with the most recently changed first. Open one with
  Enter or a click, start a new one with Ctrl+N.
- **Writing:** type away. Your post is saved when you go back to the list
  (Esc) or quit (Ctrl+Q). When you reopen a post, the cursor is where you
  left it.
- **Deleting** (Ctrl+D) moves a post to the Trash, where you can restore it
  for 30 days.

### Where your writing is kept

| Computer | Vault                                              |
| -------- | -------------------------------------------------- |
| Mac      | `~/Library/Application Support/writui/writui.db`   |
| Linux    | `~/.local/share/writui/writui.db`                  |
| Windows  | `%APPDATA%\writui\writui.db`                       |

It's a single file, so backing up means copying it somewhere safe. The copy
is just as encrypted as the original.

To keep the vault somewhere else (say, a synced folder), run
`writui --db ~/path/to/writui.db`, or make it the default by putting this in
`~/.config/writui/config.toml`:

```toml
vault = "~/path/to/writui.db"
```

## Contributing

Ideas and bug reports are welcome in
[issues](https://github.com/aakashns/writui/issues). To build writui yourself
or work on it, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

writui is free and open source, under the [MIT license](LICENSE.txt).
