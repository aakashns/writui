# writui

A writing app for the terminal. Markdown, encrypted, no nonsense.

- **Private.** All your posts live in one file, encrypted with your password.
  Without the password nobody can read them.
- **Plain markdown.** You write markdown. Each post starts with a `# Title`.
- **Mouse and keyboard.** Click, scroll and press buttons with the mouse, or
  use shortcuts. The bar at the bottom of the screen always shows the
  shortcuts for where you are, and you can click those too.
- **Fits your terminal.** It uses your terminal's own colours and font, and
  works in any modern terminal (Alacritty, Ghostty, iTerm2, and others).

> **Early days.** writui is being built in the open, a small piece at a time
> ([what's planned](docs/PLAN.md)). It's usable for writing now, but keep a
> copy of anything you can't afford to lose. And remember: **there is no way
> to recover a forgotten password.**

## Install

On macOS or Linux, paste this into a terminal:

```sh
curl -fsSL https://github.com/aakashns/writui/releases/latest/download/install.sh | bash
```

It downloads the latest writui for your computer, checks it, and puts it in
`~/.local/bin`. If that folder isn't on your `PATH` yet, it tells you the line
to add. ([Read the script](install.sh) first if you like.) Check it worked
with `writui --version`.

<details>
<summary>Or download it yourself</summary>

Every [release](https://github.com/aakashns/writui/releases/latest) has a
ready-to-run file for each computer:

| Computer                       | File                  |
| ------------------------------ | --------------------- |
| Mac with Apple Silicon (M1+)   | `writui-macos-arm64`  |
| Mac with Intel                 | `writui-macos-x86_64` |
| Linux (most PCs)               | `writui-linux-x86_64` |
| Linux (ARM, e.g. Raspberry Pi) | `writui-linux-arm64`  |

Save it as `writui` in a folder on your `PATH` and make it executable
(`chmod +x writui`). The Mac files aren't signed: if you downloaded with a
browser and macOS won't open it, run `xattr -d com.apple.quarantine writui`
once. (The install script avoids this.)

</details>

### Upgrade

```sh
writui upgrade
```

This checks for a newer version, shows what's new, and asks before replacing
writui with it (`writui upgrade --yes` skips the question). Then it asks for
your vault password (leave it empty to skip) and the new version updates your
vault right away, if it needs to. It backs the vault up first, checks the
result, and only then removes the backup. If you skip it, the update happens
the next time you unlock. Running the install script again also upgrades.

### Windows

There's no ready-made Windows version, but you can build writui yourself:

1. Install [Rust](https://rustup.rs), the
   [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   (the "Desktop development with C++" workload), and
   [Strawberry Perl](https://strawberryperl.com) (needed to build the
   encryption library).
2. In a new PowerShell window (not Git Bash, whose Perl can't build it), run:

   ```sh
   cargo install --locked --git https://github.com/aakashns/writui
   ```

The build takes a few minutes. writui's automated tests run on Windows, but
it isn't regularly tried out by hand there, so please
[report anything odd](https://github.com/aakashns/writui/issues). To upgrade,
run the same command again.

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
- **Writing:** type away. Your post saves itself as you go: a second after
  you stop typing, every few seconds while you keep typing, and when you go
  back to the list (Esc) or quit (Ctrl+Q). When you reopen a post, the
  cursor is where you left it.
- **Saves** are versions you make on purpose. Ctrl+S asks for a name (like
  a commit message) and keeps a copy of the whole post. Below the text,
  writui shows whether the post has changed since its last save.
- **History** (Ctrl+R) lists a post's saves. Open one to read it, and press
  Enter to restore it into your post (Ctrl+Z undoes that).
- **Undo** with Ctrl+Z and **redo** with Ctrl+Y (or Ctrl+Shift+Z, in
  terminals that tell it apart from Ctrl+Z). Undo goes back a word at a
  time while typing.
- **Selecting:** drag with the mouse, double-click a word, triple-click a
  paragraph, or hold Shift while moving (Ctrl+A selects everything).
  Typing or Backspace replaces what's selected. Ctrl+C copies, Ctrl+X cuts
  and Ctrl+V pastes, using the system clipboard (your terminal's own paste,
  like Cmd+V, works too). Esc drops the selection.
- **Moving by word and paragraph:** Option+Left/Right (Ctrl+Left/Right on
  Linux and Windows) jump by word, Option+Up/Down (Ctrl+Up/Down) by
  paragraph. Add Shift to select as you go.
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
